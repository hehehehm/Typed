# LayoutKeeper на телефонах (Android / iOS)

Десктопный перехват клавиатуры на телефонах **невозможен**: ни Android, ни iOS не дают приложению
видеть нажатия в чужих приложениях или печатать в них. Единственный легальный путь — **своя
клавиатура (IME)**. В этом каталоге только архитектура; работающего мобильного приложения в
репозитории нет, и мы не делаем вид, что оно есть.

## Android — собственная клавиатура (InputMethodService)

Обе функции реализуемы полностью:

* **Память по приложениям**: в `onStartInput(EditorInfo info, …)` есть `info.packageName` —
  это и есть app id. Клавиатура сама рисует раскладку, поэтому «переключение» = показать en или ru.
* **Автоисправление**: клавиатура знает каждую нажатую клавишу, при пробеле вызывает ядро и через
  `InputConnection.deleteSurroundingText(n, 0)` + `commitText(...)` заменяет слово.
* **Пароли**: `EditorInfo.inputType` с `TYPE_TEXT_VARIATION_PASSWORD` / `VISIBLE_PASSWORD` /
  `WEB_PASSWORD` → автоисправление выключается, буфер не ведётся.

Архитектура:

```
Kotlin: LayoutKeeperIme : InputMethodService
   ├─ KeyboardView (своя, en/ru)
   ├─ per-app prefs (SharedPreferences / DataStore, локально)
   └─ JNI / uniffi ──► liblayout_keeper_core.so (cargo-ndk, crate-type cdylib)
                         Engine::feed / decide / manual_convert
```

Сборка ядра: `cargo install cargo-ndk && cargo ndk -t arm64-v8a -o app/src/main/jniLibs build --release -p layout_keeper_core`.
Сейчас ядро экспортирует простой C ABI (`lk_autocorrect_word`, `lk_convert_layout`, `lk_free` в
`crates/core/src/ffi.rs`); для Kotlin удобнее добавить `uniffi`-обёртку над `Engine`.

Ограничение: поскольку это физически другая клавиатура, «зуфсу вместо peace» на телефоне случается
реже — полезнее авто-выбор раскладки по приложению и ручная кнопка конвертации слова.

## iOS — Custom Keyboard Extension

* Системные хуки невозможны. Только своё расширение клавиатуры (`UIInputViewController`).
* Текст меняется через `textDocumentProxy.deleteBackward()` + `insertText(_:)`.
* Определить приложение-хост **нельзя** официальным API — память раскладки по приложениям на iOS
  можно делать лишь приблизительно (например, по `keyboardType`/`textContentType` поля).
* В полях паролей и защищённых полях iOS сама подменяет клавиатуру на системную.
* Ядро подключается как `staticlib` (`cargo build --target aarch64-apple-ios -p layout_keeper_core`)
  через C ABI или `uniffi` → Swift.
* «Полный доступ» (Full Access) не нужен — сеть ядру не требуется, и её не должно быть.
