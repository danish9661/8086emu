// Lightweight i18n scaffold: UI strings live in window.I18N keyed by language
// code. Elements opt in with a `data-i18n="key"` attribute; applyI18n() swaps
// their textContent. The language choice is persisted in localStorage.
// Add a language by extending I18N below and adding an <option> in index.html.
window.I18N = {
  en: {
    example: 'Example', assemble: 'Assemble', step: 'Step', stepOver: 'Over',
    stepBack: 'Back', run: 'Run', stop: 'Stop', reset: 'Reset',
    saveState: 'Save', loadState: 'Load', share: 'Share',
    registers: 'Registers', flags: 'Flags', disasm: 'Disassembly', watch: 'Watch',
    memory: 'Memory', memmap: 'Memory map', ports: 'I/O',
    peripherals: 'Peripheral registers', output: 'Output', gfxscreen: 'Graphics screen (INT 10h)',
  },
  es: {
    example: 'Ejemplo', assemble: 'Ensamblar', step: 'Paso', stepOver: 'Paso',
    stepBack: 'Atrás', run: 'Ejecutar', stop: 'Detener', reset: 'Reiniciar',
    saveState: 'Guardar', loadState: 'Cargar', share: 'Enlace',
    registers: 'Registros', flags: 'Banderas', disasm: 'Desensamblado', watch: 'Vigilar',
    memory: 'Memoria', memmap: 'Mapa de memoria', ports: 'E/S',
    peripherals: 'Periféricos', output: 'Salida', gfxscreen: 'Pantalla gráfica (INT 10h)',
  },
  de: {
    example: 'Beispiel', assemble: 'Assemblieren', step: 'Schritt', stepOver: 'Überspr.',
    stepBack: 'Zurück', run: 'Start', stop: 'Stopp', reset: 'Zurücksetzen',
    saveState: 'Speichern', loadState: 'Laden', share: 'Teilen',
    registers: 'Register', flags: 'Flags', disasm: 'Disass.', watch: 'Überw.',
    memory: 'Speicher', memmap: 'Speicherkarte', ports: 'E/A',
    peripherals: 'Peripherie', output: 'Ausgabe', gfxscreen: 'Grafikbildschirm (INT 10h)',
  },
  fr: {
    example: 'Exemple', assemble: 'Assembler', step: 'Pas', stepOver: 'Passer',
    stepBack: 'Retour', run: 'Lancer', stop: 'Arrêter', reset: 'Réinit.',
    saveState: 'Sauver', loadState: 'Charger', share: 'Partager',
    registers: 'Registres', flags: 'Drapeaux', disasm: 'Désass.', watch: 'Surveiller',
    memory: 'Mémoire', memmap: 'Carte mémoire', ports: 'E/S',
    peripherals: 'Périphériques', output: 'Sortie', gfxscreen: 'Écran graphique (INT 10h)',
  },
  hi: {
    example: 'उदाहरण', assemble: 'असेंबल', step: 'चरण', stepOver: 'ओवर',
    stepBack: 'पीछे', run: 'चलाएँ', stop: 'रोकें', reset: 'रीसेट',
    saveState: 'सहेजें', loadState: 'लोड', share: 'लिंक',
    registers: 'रजिस्टर', flags: 'फ्लैग', disasm: 'डिसअसेंबल', watch: 'वॉच',
    memory: 'मेमोरी', memmap: 'मेमोरी मानचित्र', ports: 'I/O',
    peripherals: 'परिधीय', output: 'आउटपुट', gfxscreen: 'ग्राफिक्स स्क्रीन (INT 10h)',
  },
};

window.applyI18n = function (lang) {
  lang = lang || localStorage.getItem('mcu_lang') || 'en';
  const dict = window.I18N[lang] || window.I18N.en;
  document.querySelectorAll('[data-i18n]').forEach((el) => {
    const k = el.getAttribute('data-i18n');
    if (dict[k]) el.textContent = dict[k];
  });
};

(function () {
  const sel = document.getElementById('lang');
  if (!sel) return;
  const saved = localStorage.getItem('mcu_lang') || 'en';
  sel.value = saved;
  sel.addEventListener('change', () => {
    localStorage.setItem('mcu_lang', sel.value);
    window.applyI18n(sel.value);
  });
  window.applyI18n(saved);
})();
