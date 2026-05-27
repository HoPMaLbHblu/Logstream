# Руководство пользователя

## 1. Установка

Нужен установленный Rust toolchain.

```bash
cd logstream-ratatui
cargo build --release
```

Готовый бинарник появится в `target/release/logstream-ratatui`.

## 2. Подготовка логов

Приложение может читать один или несколько файлов. Если файла ещё нет, программа
создаст его и будет ждать новые строки.

```bash
touch examples/app.log examples/auth.log
```

Поддерживаемый формат:

```text
INFO message service=core source=system
WARN slow request service=api source=nginx latency=900
ERROR login failed service=auth source=gateway user=admin
```

## 3. Первый запуск

```bash
cargo run -- start examples/app.log --config examples/log-config.yaml --session demo
```

Можно указать несколько файлов:

```bash
cargo run -- start examples/app.log examples/auth.log --config examples/log-config.yaml --session prod-watch
```

## 4. Проверка потокового режима

Оставьте приложение открытым и в другом терминале добавляйте строки:

```bash
echo "INFO user logged in service=auth user=admin source=gateway" >> examples/app.log
echo "ERROR login failed service=auth user=admin source=gateway" >> examples/app.log
echo "ERROR login failed service=auth user=admin source=gateway" >> examples/app.log
```

После превышения порога правила появится предупреждение.

## 4.1 Реалистичный полигон

Для полноценной демонстрации используйте готовую папку `examples/realistic`.
Там есть четыре лога:

- `api.log` - API gateway;
- `auth.log` - сервис авторизации;
- `payments.log` - платежный сервис;
- `worker.log` - очередь фоновых задач.

Запуск анализатора:

```bash
cargo run -- start \
  examples/realistic/logs/api.log \
  examples/realistic/logs/auth.log \
  examples/realistic/logs/payments.log \
  examples/realistic/logs/worker.log \
  --config examples/realistic/log-config.yaml \
  --session realistic-demo \
  --report examples/realistic/reports/live-report.txt
```

Во втором терминале:

```bash
./examples/realistic/generate_live_logs.sh
```

Скрипт имитирует нормальный поток и аварийные всплески. В интерфейсе должны
появляться предупреждения по правилам `auth-bruteforce-suspect`,
`payment-timeouts`, `api-error-burst` и `worker-retry-storm`.

## 5. Управление в TUI

- `1` - поток логов;
- `2` - статистика;
- `3` - предупреждения;
- `s` - сохранить сессию;
- `r` - записать отчёт;
- `h` - открыть или закрыть помощь;
- `q` - остановить анализ.

## 6. Консольный режим без TUI

```bash
cargo run -- start examples/app.log --config examples/log-config.yaml --session demo --no-tui
```

Остановка: `Ctrl+C`.

## 7. Сессии

Посмотреть сохранённые сессии:

```bash
cargo run -- sessions
```

Посмотреть последнюю сохранённую статистику:

```bash
cargo run -- stats --session demo
```

## 8. Отчёты

Отчёт можно указать при запуске:

```bash
cargo run -- start examples/app.log --config examples/log-config.yaml --session demo --report reports/demo.txt
```

Или выгрузить из сохранённой сессии:

```bash
cargo run -- stats --session demo --report reports/demo.txt
```

## 9. Конфигурация

Создать конфигурацию по умолчанию:

```bash
cargo run -- config init log-config.yaml
```

Посмотреть конфигурацию:

```bash
cargo run -- config show log-config.yaml
```

Основные поля:

- `workers` - количество параллельных обработчиков;
- `read_from_end` - начинать чтение с конца файла;
- `poll_interval_ms` - частота проверки новых строк;
- `retained_events` - сколько последних событий хранить в памяти;
- `filters` - фильтр уровней, ключевого слова и источника;
- `rules` - правила подсчёта, предупреждений и игнорирования.

## 11. Фильтрация логов

Фильтрация реализована по всем параметрам из задания:

- уровень события;
- наличие ключевого слова;
- источник события.

Через конфиг:

```yaml
filters:
  levels: ["ERROR", "WARN"]
  keyword: "login failed"
  source: auth-service
```

Через CLI:

```bash
cargo run -- start examples/realistic/logs/auth.log \
  --config examples/realistic/log-config.yaml \
  --level ERROR \
  --keyword "login failed" \
  --source auth-service
```

Можно указать несколько уровней:

```bash
cargo run -- start examples/realistic/logs/api.log --level WARN --level ERROR
cargo run -- start examples/realistic/logs/api.log --level WARN,ERROR
```


