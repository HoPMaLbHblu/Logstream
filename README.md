# Logstream

CLI/TUI приложение для потокового анализа логов в реальном времени.

## Возможности

- чтение одного или нескольких растущих файлов;
- потоковая обработка без загрузки файла целиком в память;
- парсинг LEVEL message key=value;
- фильтры по уровню, ключевому слову и источнику;
- правила count, warn, ignore;
- обнаружение всплесков ошибок и повторяющихся событий;
- параллельные worker-задачи;
- сессии и восстановление статистики;
- вывод в консоль, TUI и файл отчёта.

## Запуск

```
cargo run -- start examples/app.log --config examples/log-config.yaml --session demo
```


Консольный режим:

```
cargo run -- start examples/app.log --config examples/log-config.yaml --session demo --no-tui
```


## Реалистичная проверка

```
cargo run -- start \
  examples/realistic/logs/api.log \
  examples/realistic/logs/auth.log \
  examples/realistic/logs/payments.log \
  examples/realistic/logs/worker.log \
  --config examples/realistic/log-config.yaml \
  --session realistic-demo \
  --report examples/realistic/reports/live-report.txt
```




## Управление

- 1 - поток логов;
- 2 - статистика;
- 3 - предупреждения;
- r - записать отчёт;
- q - остановить анализ.

## Сессии

```
cargo run -- sessions
cargo run -- stats --session demo
```


## Конфигурация

```
cargo run -- config init log-config.yaml
cargo run -- config show examples/log-config.yaml
```