# CLAUDE.md — minirun (learning-by-doing)

Я (Ihor) — Java-dev, изучаю Rust через этот edu-project.
Это solo-проект **для self-education по Rust**, не для портфолио, не
production. Главная цель — личный Rust fluency + разговорный материал
на будущих интервью. Не копипаст — учебная среда.

## Правила для Claude в этом репо

1. **НЕ пиши код за меня сразу.** Задавай наводящие вопросы, предлагай
   направление, но даю мне написать и запнуться самому.
2. **Если я прошу готовое решение** — сначала «попробуй сам, если
   застрянешь — покажу». Готовое решение только после моей второй
   просьбы или прямой фразы «просто покажи».
3. **Explain Rust-паттерны с Java-параллелями.** Я лучше понимаю когда
   вижу эквивалент в Java (`Arc<Mutex<T>>` ≈ `synchronized`, `?` ≈
   checked exceptions, GAT (Generic Associated Type) ≈ generic
   associated typing которого в Java нет, и т.д.).
4. **Расшифровывай сокращения** в месте где они встречаются первый раз
   (CLI, HTTP, JVM, JSON, RAII, FFI, p50/p99 и т.д.).
5. **Используй file:line references** в объяснениях (`src/main.rs:42`).
6. **Пиши на русском** если я пишу на русском, на английском — если на
   английском.
7. **Не вводи фреймворки/крейты без обсуждения.** В self-education
   крейте каждая новая dep — риск, что я напишу меньше кода руками.
   Прежде чем добавлять в `Cargo.toml`, обоснуй что `std` + мелкая
   внешняя dep не справляются.

## Что за проект

**minirun** = Rust CLI (Command Line Interface) который измеряет
cold-start любого Docker-контейнера с HTTP (HyperText Transfer
Protocol) health endpoint:

- Launches container (`docker run -d`)
- Polls `--wait-url` до 200 OK
- Timing через `std::time::Instant` (monotonic)
- Repeats N раз, reports p50 / p99 / mean / stddev
- Optional JSON (JavaScript Object Notation) output для CI
  (Continuous Integration) pipelines

Language-agnostic — работает с nginx, Postgres, Spring Boot, Node.js.
Этот `minirun` обобщённый, не JVM-specific, чтобы руки размять по
async stack без привязки к одной экосистеме.

**Rust-навыки которые тренируем:** `clap` derive, `tokio` async
runtime, `reqwest` client, `std::process::Command`, `Instant`
/ `Duration`, `#[derive(Serialize)]` для JSON, `thiserror` для proper
error enums, integration tests через `tempfile` / реальный `docker`,
GitHub Actions CI.

## Curriculum (пошаговый план)

Полный curriculum с 10 фазами (1 фаза = 1 вечер) в Google Drive:

<https://drive.google.com/file/d/1Z3QD5ARFI9AsaIhf-zd1txvCFPDKCXGi/view>

**Текущая фаза:** Phase 3
<!-- 👆 обнови здесь когда перейдёшь на следующую фазу -->

## База объяснений / диаграмм (общая на три self-edu проекта)

<https://drive.google.com/drive/folders/1xe-DCcgL7-mtTLkiYrzrDjwKhRtIYKN->
(папка `rust-work`)

Полезно для этого проекта:

- `rust-patterns-catalog.md` — `Arc<Mutex<T>>`, `?` operator, thiserror,
  async/await, `#[derive(Serialize)]` паттерны. База.
- `terminology.md` — все сокращения (JVM, FFI, GAT, RAII, OUI, CIDR и
  др.) с расшифровками.
- `rust-selfedu-README.md` — зачем все три проекта и как они
  позиционируются (не портфолио, а self-education).

## Workflow

1. Открыл VS Code → Claude Code extension в сайдбаре читает этот
   CLAUDE.md.
2. Прочитал цель текущей Phase в curriculum.
3. Пробую сам. Запнулся — спрашиваю Claude.
4. Phase закончена → `cargo clippy -- -D warnings && cargo fmt --check`
   → `git commit -am "phase N: <что сделал>"` → обнови
   `**Текущая фаза**` выше.

## Качество (перед push'ем)

- [ ] `cargo test` зелёный (unit + integration)
- [ ] `cargo clippy --all-targets -- -D warnings` чистый
- [ ] `cargo fmt --all -- --check` ok
- [ ] (с Phase 7+) JSON output парсится `jq`
- [ ] README обновлён если changed public API

## Что НЕ делать

- Не добавлять блоатные deps (hyper, actix, warp) — цель self-edu
  показать себе что знаю низкоуровневый async/HTTP стек. `tokio +
  reqwest` достаточно.
- Не копировать код из `rust-nano-vm/crates/bench/*` — чтобы я его сам
  попял/набрал.
- Не хардкодить Spring Boot / JVM — это **universal** cold-start
  measurer. Примеры в README покажи для nginx + Spring Boot +
  Postgres.
- Не делать merge/push до прохождения качества-чеклиста выше.
- Не коммитить `.env`, `target/`, `.idea/`, `.vscode/settings.json`
  (личные), результаты бенчмарков (`bench-*.json`).
