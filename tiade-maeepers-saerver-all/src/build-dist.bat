


REM Define source file and destination file
set "SOURCE_FILE=C:\BashAssetFinal\target\release\THE-META_GAME-Magi-Tek_Tek-Magi-Engiane.exe"
set "DEST_FILE=C:\BashAssetFinal-build\THE-META_GAME-Magi-Tek_Tek-Magi-Engiane.exe"

if exist "%SOURCE_FILE%" (
    echo Copying file "%SOURCE_FILE%" to "%DEST_FILE%".
    copy "%SOURCE_FILE%" "%DEST_FILE%" /Y
) else (
    echo File "%SOURCE_FILE%" does not exist.
)
