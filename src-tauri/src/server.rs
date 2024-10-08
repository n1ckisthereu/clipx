use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tauri::State;

use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};

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

        tokio::spawn(async move {
            if let Err(e) = run_server(password, state_clone).await {
                eprintln!("Error running the server: {}", e);
            }
        });
        
        println!("Server started successfully"); // Debug log
        Ok(())
    } else {
        println!("Server start failed - already running"); // Debug log
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
