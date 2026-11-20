# Как мы разрабатываем игру вместе

Проект: **SCP: Secret Laboratory — 3D**, Rust + Bevy 0.15.
Работаем через git + OpenCode у каждого на своём ПК.

## Раскладка

```
scp/
  scp_sl_3d/          # сам проект Bevy (Cargo.toml, src, assets)
  tools/              # capture_game.ps1, make_package.ps1
  releases/           # готовые zip для передачи друзьям
  docs/screenshots/   # скриншоты
  legacy/             # старая 2D-версия на Python и HTML-страница
  README.md           # про игру
  COLLAB.md           # этот файл
```

## Сборка и запуск

```powershell
cd scp_sl_3d
cargo run              # отладка
cargo build --release  # релиз
```

Тулчейн: Rust stable `x86_64-pc-windows-gnu` + MinGW GCC (WinLibs).
В `.cargo/config.toml` прописан абсолютный путь к gcc — на новом ПК его
нужно поправить (или переключиться на MSVC-тулчейн).

## Гит-процесс

1. У каждого свой клон репозитория, работа в своей ветке:
   ```powershell
   git pull
   git switch -c feature/название
   ```
2. Одна задача — один коммит, понятные сообщения (можно по-русски).
3. Перед пушем: `cargo build`, чтобы не пушить сломанное.
4. Ветки не смешиваем напрямую, сначала `git pull --rebase main`.

OpenCode умеет создавать отдельные **worktree** — удобно, когда один
делает новый SCP, а второй правит карту/UI одновременно:
в сессии OpenCode попроси «создай worktree для ветки ...».

## Правила

- `target/` не коммитить (в `.gitignore`).
- Большие ассеты/скриншоты — через `releases/` или ссылку, не в репо.
- Не переписывать чужие правки молча: сначала `git log`/`git blame`.
- Перед большим рефакторингом — написать в общий чат, кто что трогает.

## Где  remote

Если репозиторий ещё не создан на GitHub/GitLab/Codeberg: создать пустой
приватный репозиторий и выполнить локально:

```powershell
git remote add origin <ссылка-на-репозиторий>
git push -u origin main
```
