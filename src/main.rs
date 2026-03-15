use linemux::MuxedLines;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout};
use rcon::Connection;
use chrono::Local;
use axum::{routing::get, Router, Json, extract::State};

#[derive(Deserialize, Clone)]
struct Config {
    log_path: String,
    server_address: String, // Новое поле для SLP
    rcon_address: String,
    rcon_password: String,
    chunk_limit: u32,
}

// Наше состояние, которое отдается по JSON на порту 9999
#[derive(Serialize, Clone)]
struct ServerStatus {
    is_online: bool,
    is_restarting: bool,
    player_count: usize, // Было i32, стало usize
    players: Vec<String>,
}

type SharedStatus = Arc<RwLock<ServerStatus>>;

fn now() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

// Обработчик для axum, возвращающий JSON
async fn get_status(State(state): State<SharedStatus>) -> Json<ServerStatus> {
    let status = state.read().await.clone();
    Json(status)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_str = fs::read_to_string("config.toml")
        .expect("Не удалось найти или прочитать файл config.toml");
    let config: Config = toml::from_str(&config_str)
        .expect("Ошибка парсинга config.toml");

    // 1. Создаем разделяемое состояние
    let status = Arc::new(RwLock::new(ServerStatus {
        is_online: false,
        is_restarting: false,
        player_count: 0,
        players: vec![],
    }));

    // 2. Запускаем HTTP-сервер (axum) на порту 9999
    let app = Router::new()
        .route("/status", get(get_status))
        .with_state(status.clone());

    tokio::spawn(async move {
        println!("[{}] Запуск HTTP API на 0.0.0.0:9999...", now());
        let listener = tokio::net::TcpListener::bind("0.0.0.0:9999").await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

// 3. Запускаем фоновый поллер (SLP)
    let poller_status = status.clone();
    let slp_address = config.server_address.clone();
    
    tokio::spawn(async move {
        let addr: SocketAddr = slp_address.parse().expect("Неверный формат server_address");
        let hostname = addr.ip().to_string();
        let port = addr.port();
        
        loop {
            // Оборачиваем в таймаут и подключение, и сам пинг
            let ping_result = timeout(Duration::from_secs(3), async {
                // 1. Устанавливаем TCP-соединение сами
                let mut stream = tokio::net::TcpStream::connect(&addr).await?;
                // 2. Передаем поток в craftping
                craftping::tokio::ping(&mut stream, &hostname, port).await
            }).await;

            match ping_result {
                Ok(Ok(pong)) => {
                    let mut st = poller_status.write().await;
                    st.is_online = true;
                    st.is_restarting = false; 
                    st.player_count = pong.online_players;
                    
                    if let Some(sample) = pong.sample {
                        st.players = sample.into_iter().map(|p| p.name).collect::<Vec<String>>();
                    } else {
                        st.players.clear();
                    }
                }
                _ => {
                    // Ошибка таймаута, подключения или протокола
                    let mut st = poller_status.write().await;
                    st.is_online = false;
                    st.player_count = 0;
                    st.players.clear();
                }
            }
            sleep(Duration::from_secs(5)).await;
        }
    });

    println!("[{}] AOC Monitor запущен. Чтение логов: {}", now(), config.log_path);

    let chunk_regex = Regex::new(r"\|-\sLevelChunk\s\(minecraft\):\s(\d+)")?;
    let mut lines = MuxedLines::new()?;
    lines.add_file(&config.log_path).await?;

    // 4. Основной цикл: чтение логов и проверка утечек
    while let Ok(Some(line)) = lines.next_line().await {
        let text = line.line();

        if let Some(captures) = chunk_regex.captures(text) {
            if let Some(chunk_str) = captures.get(1) {
                if let Ok(chunks) = chunk_str.as_str().parse::<u32>() {
                    if chunks > config.chunk_limit {
                        println!("[{}] КРИТИЧЕСКАЯ УТЕЧКА! Чаков: {}. Лимит: {}.", now(), chunks, config.chunk_limit);
                        
                        // Меняем статус на "в рестарте" для API
                        {
                            let mut st = status.write().await;
                            st.is_restarting = true;
                        }

                        trigger_restart(config.clone()).await;
                        
                        println!("[{}] Ожидание перезапуска сервера...", now());
                        // Спим подольше, чтобы дать серверу время выключиться
                        // Когда он включится, SLP-поллер сам сбросит is_restarting в false
                        sleep(Duration::from_secs(45)).await; 
                    }
                }
            }
        }
    }

    Ok(())
}

async fn trigger_restart(config: Config) {
    match Connection::builder()
        .enable_minecraft_quirks(true)
        .connect(config.rcon_address.as_str(), config.rcon_password.as_str())
        .await
    {
        Ok(mut conn) => {
            println!("[{}] Отправка предупреждения в чат...", now());
            let _ = conn.cmd("/say Внимание! Критическая утечка памяти. Рестарт сервера через 1 минуту! Пожалуйста, спрячьтесь в безопасное место.").await;
            sleep(Duration::from_secs(60)).await;
            
            let _ = conn.cmd("/say Выполняется перезагрузка...").await;
            sleep(Duration::from_secs(2)).await;

            println!("[{}] Отправка команды /stop...", now());
            match timeout(Duration::from_secs(2), conn.cmd("/stop")).await {
                Ok(Ok(_)) => println!("[{}] Команда /stop отработала подозрительно быстро.", now()),
                Ok(Err(e)) => eprintln!("[{}] Ошибка RCON при отправке /stop: {}", now(), e),
                Err(_) => println!("[{}] Сервер ушел в шатдаун и не ответил. Разрываем соединение.", now()),
            }
        }
        Err(e) => eprintln!("[{}] Ошибка подключения к RCON: {}", now(), e),
    }
}