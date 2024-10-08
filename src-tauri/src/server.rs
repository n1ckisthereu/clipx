use std::io::{Cursor};
use std::sync::Arc;
use arboard::{Clipboard, ImageData};
use image::{DynamicImage, ImageBuffer, ImageOutputFormat, RgbaImage};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tauri::State;

use tokio::net::TcpListener;
use tokio::time::{self, timeout, Duration};

#[derive(Default)]
pub struct ServerState {
    is_running: Arc<Mutex<bool>>,
    clients: Arc<Mutex<Vec<Arc<Mutex<tokio::net::TcpStream>>>>>,
    authenticated_clients: Arc<Mutex<Vec<Arc<Mutex<tokio::net::TcpStream>>>>>,
}

impl ServerState {
    pub fn new() -> Self {
        println!("Creating new ServerState");
        ServerState {
            is_running: Arc::new(Mutex::new(false)),
            clients: Arc::new(Mutex::new(Vec::new())),
            authenticated_clients: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn broadcast(&self, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut clients = self.authenticated_clients.lock().await;
        let mut clients_to_remove = vec![];

        for client in clients.iter() {
            let mut client_socket = client.lock().await;
            if let Err(e) = client_socket.write_all(message.as_bytes()).await {
                eprintln!("Error sending message to client: {}", e);
                if let Err(close_error) = client_socket.shutdown().await {
                    eprintln!("Error closing socket: {}", close_error);
                }
                clients_to_remove.push(Arc::clone(client));
            }
        }

        clients.retain(|client| !clients_to_remove.iter().any(|c| Arc::ptr_eq(c, client)));

        Ok(())
    }
}


#[tauri::command]
pub async fn get_server_status(state: State<'_, ServerState>) -> Result<bool, String> {
    let is_running = *state.is_running.lock().await;
    println!("Current server status: {}", is_running);
    Ok(is_running)
}

#[tauri::command]
pub async fn start_server(state: State<'_, ServerState>, password: String) -> Result<(), String> {
    let mut is_running = state.is_running.lock().await;
    println!("Attempting to start server. Current status: {}", *is_running);
    
    if !*is_running {
        *is_running = true;
        let password = Arc::new(password);
        
        let state_clone = Arc::new(ServerState {
            is_running: Arc::clone(&state.is_running),
            clients: Arc::clone(&state.clients),
            authenticated_clients: Arc::clone(&state.authenticated_clients),
        });

        let server_state = Arc::clone(&state_clone);
        tokio::spawn(async move {
            if let Err(e) = run_server(password, server_state).await {
                eprintln!("Error running the server: {}", e);
            }
        });

        let monitor_state = Arc::clone(&state_clone);
        tokio::spawn(async move {
            monitor_clipboard(monitor_state).await;
        });
        
        println!("Server and clipboard monitor started successfully");
        Ok(())
    } else {
        println!("Server start failed - already running");
        Err("Server start failed - already running".into())
    }
}

#[tauri::command]
pub async fn stop_server(state: State<'_, ServerState>) -> Result<(), String> {
    let mut is_running = state.is_running.lock().await;
    println!("Attempting to stop server. Current status: {}", *is_running); // Debug log
    
    if *is_running {
        *is_running = false;
        println!("Server stopped successfully"); // Debug log

        let mut clients = state.clients.lock().await;
        let mut authenticated_clients = state.authenticated_clients.lock().await;

        clients.clear();
        authenticated_clients.clear();
        Ok(())
    } else {
        println!("Server stop failed - not running"); // Debug log
        Err("Server is not running".into())
    }
}

async fn run_server(password: Arc<String>, state: Arc<ServerState>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind("127.0.0.1:21221").await?;
    println!("Server listening on 127.0.0.1:21221");

    const ACCEPT_TIMEOUT: Duration = Duration::from_secs(1);

    while *state.is_running.lock().await {
        match timeout(ACCEPT_TIMEOUT, listener.accept()).await {
            Ok(Ok((socket, addr))) => {
                println!("New connection from: {}", addr);
                let password = Arc::clone(&password);
                let state_clone = Arc::clone(&state);
    
                let client_socket = Arc::new(Mutex::new(socket));
    
                state_clone.clients.lock().await.push(Arc::clone(&client_socket));
    
                tokio::spawn(async move {
                    if let Err(e) = handle_client(client_socket, &password, &state_clone).await {
                        eprintln!("Error handling client: {}", e);
                    }
                });
            }
            Ok(Err(e)) => {
                eprintln!("Error accepting connection: {}", e);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(_) => {
                if !*state.is_running.lock().await {
                    println!("Server shutdown signal received");
                    break;
                }
            }
        }
    }

    println!("Server shutdown complete");
    Ok(())
}

async fn handle_client(socket: Arc<Mutex<tokio::net::TcpStream>>, password: &str, state: &Arc<ServerState>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut buffer = [0; 1024];
    let mut socket_guard = socket.lock().await;
    match timeout(Duration::from_secs(5), socket_guard.read(&mut buffer)).await {
        Ok(Ok(n)) if n > 0 => {
            let client_password = String::from_utf8_lossy(&buffer[..n]);
            println!("Received password attempt");

            let response = if client_password.trim() == password {
                println!("Correct password from client");
                let mut authenticated_clients = state.authenticated_clients.lock().await;
                authenticated_clients.push(Arc::clone(&socket));
                "Correct password! Connected.\n"
            } else {
                println!("Incorrect password from client");
                "Incorrect password!\n"
            };

            socket_guard.write_all(response.as_bytes()).await?;
        }
        Ok(Ok(_)) => {
            println!("Client disconnected");
            let mut clients_lock = state.clients.lock().await;
            clients_lock.retain(|client| !Arc::ptr_eq(client, &socket));
            let mut authenticated_clients_lock = state.authenticated_clients.lock().await;
            authenticated_clients_lock.retain(|client| !Arc::ptr_eq(client, &socket));
        }
        Ok(Err(e)) => {
            return Err(e.into());
        }
        Err(_) => {
            return Err("Client connection timed out".into());
        }
    }

    Ok(())
}

async fn monitor_clipboard(state: Arc<ServerState>) {
    let mut last_content: Option<ClipboardContent> = None;
    let mut clipboard = Clipboard::new().expect("Falha ao criar a instância do clipboard");

    while *state.is_running.lock().await {
        time::sleep(time::Duration::from_secs(1)).await;

        let content = get_clipboard_content(&mut clipboard);

        // Compare usando clone() na struct ClipboardContent
        if last_content.as_ref().map(|c| c.data.clone()) != Some(content.data.clone()) {
            last_content = Some(content.clone()); // Use the clone method here

            let json_content = serde_json::json!({
                "type": content.content_type,
                "data": content.data,
            });

            if let Err(e) = state.broadcast(&json_content.to_string()).await {
                eprintln!("Erro ao transmitir a mensagem: {}", e);
            }
        }
    }
    println!("Monitoramento do clipboard parado");
}


#[derive(Clone)]
struct ClipboardContent {
    content_type: String,
    data: String,
}

fn get_clipboard_content(clipboard: &mut Clipboard) -> ClipboardContent {
    if let Ok(text) = clipboard.get_text() {
        return ClipboardContent {
            content_type: "text".to_string(),
            data: text,
        };
    }
    
    if let Ok(image) = clipboard.get_image() {
        return ClipboardContent {
            content_type: "image".to_string(),
            data: img_to_buffer(image)
        };
    }

    ClipboardContent {
        content_type: "empty".to_string(),
        data: String::new(),
    }
}

fn img_to_buffer(image: ImageData) -> String  { 
    let image: RgbaImage = ImageBuffer::from_raw(
        image.width.try_into().unwrap(),
        image.height.try_into().unwrap(),
        image.bytes.into_owned(),
    ).unwrap();
    
    let img = DynamicImage::ImageRgba8(image);
    
    let mut buffer = Vec::new();
    img.write_to(&mut Cursor::new(&mut buffer), ImageOutputFormat::Png).unwrap();
    
    let data = base64::encode(&buffer);
    
    return data;
}

#[tauri::command]
pub async fn broadcast_message_command(state: State<'_, ServerState>, message: String) -> Result<(), String> {
    let is_running = state.is_running.lock().await;
    
    if *is_running {
        if let Err(e) = state.broadcast(&message).await {
            return Err(format!("Failed to broadcast message: {}", e));
        }
    }else {
        println!("Broadcast failed - server not running"); // Debug log
        return Err(format!("Failed to broadcast message"));
    }
  
    Ok(())
}
