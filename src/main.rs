use linemux::MuxedLines;
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use rcon::Connection;
use chrono::Local; // Импортируем работу с локальным временем

#[derive(Deserialize, Clone)]
struct Config {
    log_path: String,
    rcon_address: String,
    rcon_password: String,
    chunk_limit: u32,
}

// Вспомогательная функция, возвращающая текущее время в виде строки
fn now() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_str = fs::read_to_string("config.toml")
        .expect("Не удалось найти или прочитать файл config.toml");
    
    let config: Config = toml::from_str(&config_str)
        .expect("Ошибка парсинга config.toml. Проверьте правильность заполнения полей.");

    println!("[{}] AOC Monitor запущен. Загружен конфиг для логов: {}", now(), config.log_path);

    let chunk_regex = Regex::new(r"\|-\sLevelChunk\s\(minecraft\):\s(\d+)")?;

    let mut lines = MuxedLines::new()?;
    lines.add_file(&config.log_path).await?;

    while let Ok(Some(line)) = lines.next_line().await {
        let text = line.line();

        if let Some(captures) = chunk_regex.captures(text) {
            if let Some(chunk_str) = captures.get(1) {
                if let Ok(chunks) = chunk_str.as_str().parse::<u32>() {
                    println!("[{}] Отчет AllTheLeaks: загружено {} чанков.", now(), chunks);

                    if chunks > config.chunk_limit {
                        println!("[{}] КРИТИЧЕСКАЯ УТЕЧКА! Превышен лимит в {} чанков.", now(), config.chunk_limit);
                        
                        trigger_restart(config.clone()).await;
                        
                        println!("[{}] Ожидание перезапуска сервера...", now());
                        sleep(Duration::from_secs(30)).await;
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
            
            // Оборачиваем вызов в жесткий таймаут на 2 секунды
            match timeout(Duration::from_secs(2), conn.cmd("/stop")).await {
                Ok(Ok(_)) => println!("[{}] Команда /stop отработала подозрительно быстро.", now()),
                Ok(Err(e)) => eprintln!("[{}] Ошибка RCON при отправке /stop: {}", now(), e),
                Err(_) => println!("[{}] Сервер ушел в шатдаун и не ответил. Разрываем соединение (это нормально).", now()),
            }
            // Как только мы выходим из этого блока, переменная `conn` уничтожается (drop), 
            // закрывая сокет и освобождая застрявший поток RCON на сервере.
        }
        Err(e) => {
            eprintln!("[{}] Не удалось подключиться к RCON по адресу {}: {}", now(), config.rcon_address, e);
        }
    }
}