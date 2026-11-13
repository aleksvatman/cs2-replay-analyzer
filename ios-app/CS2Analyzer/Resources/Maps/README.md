# Радары карт (этап I3)

В репозиторий входит только калибровка (`<карта>.json`), без картинок радаров —
они из файлов игры (авторские права Valve). Без картинки приложение показывает
предупреждение вместо радара; на сами данные и тесты это не влияет.

## Откуда взяты числа калибровки

`posX`, `posY`, `scale` для `de_mirage` и `de_ancient` — это `pos_x`, `pos_y`, `scale`
из игровых файлов `resource/overviews/<карта>.txt` (формат HLTV overview). Эти текстовые
файлы (не картинки) лежат в публичном зеркале ресурсов CS2, которое тянет как
транзитивную зависимость `parser` (ядро `demoparser2`, крейт, от которого зависит
`parser-core`):

```
~/.cargo/git/checkouts/demoparser-*/*/src/csgoproto/GameTracking-CS2/game/csgo/pak01_dir/resource/overviews/de_mirage.txt
~/.cargo/git/checkouts/demoparser-*/*/src/csgoproto/GameTracking-CS2/game/csgo/pak01_dir/resource/overviews/de_ancient.txt
```

(путь зависит от версии `demoparser2`/`PROTO_COMMIT`; если файла нет, он появится
после `cargo build` в корне репозитория).

`imageSize` (1024×1024) — предположение по обычному размеру радаров CS2/CS:GO, не
проверено на реальном файле. Поправьте, если картинка окажется другого размера.

## Как добавить картинки

1. Скопировать `overviews/de_mirage.png` и `overviews/de_ancient.png` из установленной
   игры (`Counter-Strike Global Offensive.app` → `Contents/...` → `csgo/pak01_dir` —
   текстуры в игре хранятся как `.vtex_c`/скомпилированный материал, их нужно сначала
   извлечь и перевести в PNG инструментом вроде Source 2 Viewer).
2. Положить готовые `de_mirage.png` / `de_ancient.png` рядом с `.json` в эту папку
   (`ios-app/CS2Analyzer/Resources/Maps/`). Папка синхронизируемая (Xcode 16), поэтому
   PNG подхватится в сборку сам, без правки проекта.
3. Картинки в `.gitignore` — `git status` их не покажет и `git add` не подхватит.
