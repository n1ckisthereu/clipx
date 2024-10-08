use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::State;

use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{timeout, Duration};

#[derive(Default)]
pub struct ServerState(Arc<Mutex<bool>>);

impl ServerState {
    pub fn new() -> Self {
        println!("Creating new ServerState");
        ServerState(Arc::new(Mutex::new(false)))
    }
}

#[tauri::command]
pub async fn get_server_status(state: State<'_, ServerState>) -> Result<bool, String> {
    let is_running = *state.0.lock().await;
    println!("Current server status: {}", is_running);
    Ok(is_running)
}

#[tauri::command]
pub async fn start_server(state: State<'_, ServerState>, password: String) -> Result<(), String> {
    let mut is_running = state.0.lock().await;
    println!("Attempting to start server. Current status: {}", *is_running);
    
    if !*is_running {
        *is_running = true;
        let password = Arc::new(password);
        
        let state_clone = Arc::clone(&state.0);
        
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
    let mut is_running = state.0.lock().await;
    println!("Attempting to stop server. Current status: {}", *is_running); // Debug log
    
    if *is_running {
        *is_running = false;
        println!("Server stopped successfully"); // Debug log
        Ok(())
    } else {
        println!("Server stop failed - not running"); // Debug log
        Err("Server is not running".into())
    }
}

async fn run_server(password: Arc<String>, state: Arc<Mutex<bool>>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind("127.0.0.1:21221").await?;
    println!("Server listening on 127.0.0.1:21221");

    // Constante para o timeout
    const ACCEPT_TIMEOUT: Duration = Duration::from_secs(1);

    while *state.lock().await {
        match timeout(ACCEPT_TIMEOUT, listener.accept()).await {
            Ok(Ok((mut socket, addr))) => {
                println!("New connection from: {}", addr);
                let password = Arc::clone(&password);

                tokio::spawn(async move {
                    if let Err(e) = handle_client(&mut socket, &password).await {
                        eprintln!("Error handling client: {}", e);
                    }
                });
            }
            Ok(Err(e)) => {
                eprintln!("Error accepting connection: {}", e);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(_) => {
                if !*state.lock().await {
                    println!("Server shutdown signal received");
                    break;
                }
            }
        }
    }

    println!("Server shutdown complete");
    Ok(())
}

async fn handle_client(socket: &mut tokio::net::TcpStream, password: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut buffer = [0; 1024];

    match timeout(Duration::from_secs(5), socket.read(&mut buffer)).await {
        Ok(Ok(n)) if n > 0 => {
            let client_password = String::from_utf8_lossy(&buffer[..n]);
            println!("Received password attempt");

            let response = if client_password.trim() == password {
                println!("Correct password from client");
                "Correct password! Connected.\n"
            } else {
                println!("Incorrect password from client");
                "Incorrect password!\n"
            };

            socket.write_all(response.as_bytes()).await?;
        }
        Ok(Ok(_)) => {
            println!("Client disconnected");
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