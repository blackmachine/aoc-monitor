use linemux::MuxedLines;
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::time::Duration;
use tokio::time::sleep;
use rcon::Connection;

// Описываем структуру нашего конфига
// Макрос Deserialize заставит serde автоматически сопоставить поля из TOML с этой структурой
#[derive(Deserialize, Clone)]
struct Config {
    log_path: String,
    rcon_address: String,
    rcon_password: String,
    chunk_limit: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Читаем и парсим конфиг
    let config_str = fs::read_to_string("config.toml")
        .expect("❌ Не удалось найти или прочитать файл config.toml");
    
    let config: Config = toml::from_str(&config_str)
        .expect("❌ Ошибка парсинга config.toml. Проверьте правильность заполнения полей.");

    println!("🚀 AOC Monitor запущен. Загружен конфиг для логов: {}", config.log_path);

    let chunk_regex = Regex::new(r"\|-\sLevelChunk\s\(minecraft\):\s(\d+)")?;

    let mut lines = MuxedLines::new()?;
    // Используем путь из конфига
    lines.add_file(&config.log_path).await?;

    while let Ok(Some(line)) = lines.next_line().await {
        let text = line.line();

        if let Some(captures) = chunk_regex.captures(text) {
            if let Some(chunk_str) = captures.get(1) {
                if let Ok(chunks) = chunk_str.as_str().parse::<u32>() {
                    println!("📊 Отчет AllTheLeaks: загружено {} чанков.", chunks);

                    // Сравниваем с лимитом из конфига
                    if chunks > config.chunk_limit {
                        println!("⚠️ КРИТИЧЕСКАЯ УТЕЧКА! Превышен лимит в {} чанков.", config.chunk_limit);
                        
                        // Передаем клонированный конфиг в функцию RCON
                        trigger_restart(config.clone()).await;
                        
                        println!("⏳ Ожидание перезапуска сервера...");
                        sleep(Duration::from_secs(30)).await;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn trigger_restart(config: Config) {
    // Возвращаем правильный connect!
    match Connection::builder()
        .enable_minecraft_quirks(true)
        .connect(config.rcon_address.as_str(), config.rcon_password.as_str())
        .await
    {
        Ok(mut conn) => {
            println!("💬 Отправка предупреждения в чат...");
            let _ = conn.cmd("/say Внимание! Критическая утечка памяти. Рестарт сервера через 1 минуту!").await;
            
            sleep(Duration::from_secs(60)).await;
            
            let _ = conn.cmd("/say Выполняется перезагрузка...").await;
            sleep(Duration::from_secs(2)).await;

            println!("🛑 Отправка команды /stop...");
            match conn.cmd("/stop").await {
                Ok(_) => println!("✅ Команда /stop успешно отправлена."),
                Err(e) => eprintln!("❌ Ошибка при отправке /stop: {}", e),
            }
        }
        Err(e) => {
            eprintln!("❌ Не удалось подключиться к RCON по адресу {}: {}", config.rcon_address, e);
        }
    }
}