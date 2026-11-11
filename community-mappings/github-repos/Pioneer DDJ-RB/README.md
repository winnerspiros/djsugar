# Mapeo Pioneer DDJ-RB para Mixxx 2.5.x

Mapeo MIDI para la controladora Pioneer DDJ-RB, compatible con Mixxx 2.5.6.

## Instalación

1. Copia `Pioneer-DDJ-RB.midi.xml` y `Pioneer-DDJ-RB-scripts.js` en la carpeta de controladoras de Mixxx:
   - Windows: `%LOCALAPPDATA%\Mixxx\controllers`
   - Linux: `~/.mixxx/controllers`
   - macOS: `~/Library/Application Support/Mixxx/controllers`
2. Reinicia Mixxx, abre *Preferencias → Controladores*, selecciona **Pioneer DDJ-RB**, marca *Habilitado* y pulsa *Aplicar*.

El script usa `lodash.mixxx.js` y `midi-components-0.0.js`, que ya vienen incluidos con Mixxx.

## Créditos

Basado en el trabajo de Luis Mita (https://github.com/LuisMitaHL/mixxx-ddj-rb), Be y colaboradores de Mixxx.
Licencia: GPL v2 o posterior / MIT (ver cabecera del script).
