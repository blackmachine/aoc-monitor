use linemux::MuxedLines;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant}; // Добавили Instant для замера времени
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout, Instant};
use rcon::Connection;
use chrono::Local;
use axum::{routing::get, Router, Json, extract::State};

#[derive(Deserialize)]
struct Config {
    log_path: String,
    server_address: String,
    rcon_address: String,
    rcon_password: String,
    chunk_limit: u32,
    #[serde(default = "default_api_port")]
    api_port: u16,
}

fn default_api_port() -> u16 {
    9999
}

#[derive(Serialize, Clone)]
struct ServerStatus {
    is_online: bool,
    is_restarting: bool,
    player_count: usize,
    players: Vec<String>,
}

type SharedStatus = Arc<RwLock<ServerStatus>>;

fn now() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

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

<<<<<<< HEAD
=======
    // Парсим адрес SLP до запуска задачи, чтобы ошибка была видна сразу
    let slp_addr: SocketAddr = config.server_address.parse()
        .expect("Неверный формат server_address");

    let config = Arc::new(config);

>>>>>>> 7ed26ab (Reliability tweaks)
    let status = Arc::new(RwLock::new(ServerStatus {
        is_online: false,
        is_restarting: false,
        player_count: 0,
        players: vec![],
    }));

<<<<<<< HEAD
=======
    // Запускаем HTTP-сервер (axum)
    let bind_addr = format!("0.0.0.0:{}", config.api_port);
>>>>>>> 7ed26ab (Reliability tweaks)
    let app = Router::new()
        .route("/status", get(get_status))
        .with_state(status.clone());

    tokio::spawn(async move {
        println!("[{}] Запуск HTTP API на {}...", now(), bind_addr);
        let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[{}] Не удалось запустить HTTP-сервер на {}: {}", now(), bind_addr, e);
                std::process::exit(1);
            }
        };
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("[{}] HTTP-сервер упал: {}", now(), e);
            std::process::exit(1);
        }
    });

<<<<<<< HEAD
    let poller_status = status.clone();
    let slp_address = config.server_address.clone();
    
    // Фоновый поллер (SLP)
=======
    // Запускаем фоновый поллер (SLP)
    let poller_status = status.clone();
    let hostname = slp_addr.ip().to_string();
    let port = slp_addr.port();

>>>>>>> 7ed26ab (Reliability tweaks)
    tokio::spawn(async move {
        loop {
            let ping_result = timeout(Duration::from_secs(3), async {
<<<<<<< HEAD
                let mut stream = tokio::net::TcpStream::connect(&addr).await?;
=======
                let mut stream = tokio::net::TcpStream::connect(&slp_addr).await?;
>>>>>>> 7ed26ab (Reliability tweaks)
                craftping::tokio::ping(&mut stream, &hostname, port).await
            }).await;

            match ping_result {
                Ok(Ok(pong)) => {
                    let mut st = poller_status.write().await;
                    st.is_online = true;
<<<<<<< HEAD
                    // ВАЖНО: Мы больше не сбрасываем здесь st.is_restarting = false
=======
                    st.is_restarting = false;
>>>>>>> 7ed26ab (Reliability tweaks)
                    st.player_count = pong.online_players;

                    if let Some(sample) = pong.sample {
                        st.players = sample.into_iter().map(|p| p.name).collect();
                    } else {
                        st.players.clear();
                    }
                }
                _ => {
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

<<<<<<< HEAD
    // Основной цикл: чтение логов и проверка утечек
    while let Ok(Some(line)) = lines.next_line().await {
        let text = line.line();

        if let Some(captures) = chunk_regex.captures(text) {
            if let Some(chunk_str) = captures.get(1) {
                if let Ok(chunks) = chunk_str.as_str().parse::<u32>() {
                    if chunks > config.chunk_limit {
                        
                        // 1. Защита от спама в логах: проверяем, не начали ли мы уже рестарт
                        let is_already_restarting = status.read().await.is_restarting;
                        if is_already_restarting {
                            continue; 
                        }

                        println!("[{}] КРИТИЧЕСКАЯ УТЕЧКА! Чаков: {}. Лимит: {}.", now(), chunks, config.chunk_limit);
                        
                        // Засекаем время начала всей процедуры
                        let start_reboot_time = Instant::now();

                        {
                            let mut st = status.write().await;
                            st.is_restarting = true;
                        }

                        // Вызываем RCON скрипт (он ждет 60 секунд внутри)
                        trigger_restart(config.clone()).await;
                        
                        println!("[{}] Ожидание полного выключения сервера...", now());
                        
                        // Ждем, пока поллер не зафиксирует оффлайн (таймаут 2 минуты на случай зависания)
                        let _ = timeout(Duration::from_secs(120), async {
                            while status.read().await.is_online {
                                sleep(Duration::from_secs(2)).await;
                            }
                        }).await;

                        // Сервер упал, засекаем чистое время простоя
                        let downtime_start = Instant::now();
                        println!("[{}] Сервер выключен. Ожидание запуска...", now());

                        // Ждем, пока сервер поднимется обратно (таймаут 10 минут)
                        let _ = timeout(Duration::from_secs(600), async {
                            while !status.read().await.is_online {
                                sleep(Duration::from_secs(5)).await;
                            }
                        }).await;
                        
                        let downtime_duration = downtime_start.elapsed().as_secs();
                        let total_reboot_duration = start_reboot_time.elapsed().as_secs();

                        println!("[{}] Сервер снова online! Время простоя: {} сек. Общее время перезагрузки: {} сек.", 
                            now(), downtime_duration, total_reboot_duration);

                        // Всё прошло успешно, снимаем флаг
                        {
                            let mut st = status.write().await;
                            st.is_restarting = false;
                        }
=======
    // Кулдаун после рестарта: игнорируем срабатывания в течение 3 минут
    let restart_cooldown = Duration::from_secs(180);
    let mut last_restart = Instant::now() - restart_cooldown;

    // Основной цикл: чтение логов и проверка утечек чанков
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            result = lines.next_line() => {
                let Some(line) = result? else { break };
                let text = line.line();

                if let Some(captures) = chunk_regex.captures(text) {
                    if let Ok(chunks) = captures[1].parse::<u32>() {
                        if chunks > config.chunk_limit && last_restart.elapsed() > restart_cooldown {
                            println!("[{}] КРИТИЧЕСКАЯ УТЕЧКА! Чанков: {}. Лимит: {}.", now(), chunks, config.chunk_limit);

                            {
                                let mut st = status.write().await;
                                st.is_restarting = true;
                            }

                            trigger_restart(&config).await;
                            last_restart = Instant::now();

                            println!("[{}] Ожидание перезапуска сервера...", now());
                        }
>>>>>>> 7ed26ab (Reliability tweaks)
                    }
                }
            }
            _ = &mut shutdown => {
                println!("[{}] Получен сигнал завершения, выход...", now());
                break;
            }
        }
    }

    Ok(())
}

async fn trigger_restart(config: &Config) {
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
