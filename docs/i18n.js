// Lightweight i18n scaffold: UI strings live in window.I18N keyed by language
// code. Elements opt in with a `data-i18n="key"` attribute; applyI18n() swaps
// their textContent. The language choice is persisted in localStorage.
// Add a language by extending I18N below and adding an <option> in index.html.
window.I18N = {
  en: {
    example: 'Example', assemble: 'Assemble', step: 'Step', stepOver: 'Step-Over',
    stepBack: 'Step-Back', run: 'Run', stop: 'Stop', reset: 'Reset',
    saveState: 'Save State', loadState: 'Load State', share: 'Share Link',
    registers: 'Registers', flags: 'Flags', disasm: 'Disassembly', watch: 'Watch',
    memory: 'Memory', memmap: 'Memory map', ports: 'I/O Ports',
    peripherals: 'Peripheral registers', output: 'Program output',
  },
  es: {
    example: 'Ejemplo', assemble: 'Ensamblar', step: 'Paso', stepOver: 'Paso-over',
    stepBack: 'Paso-atrás', run: 'Ejecutar', stop: 'Detener', reset: 'Reiniciar',
    saveState: 'Guardar estado', loadState: 'Cargar estado', share: 'Enlace',
    registers: 'Registros', flags: 'Banderas', disasm: 'Desensamblado', watch: 'Vigilar',
    memory: 'Memoria', memmap: 'Mapa de memoria', ports: 'Puertos E/S',
    peripherals: 'Periféricos', output: 'Salida del programa',
  },
  de: {
    example: 'Beispiel', assemble: 'Assemblieren', step: 'Schritt', stepOver: 'Überspringen',
    stepBack: 'Rückgängig', run: 'Start', stop: 'Stopp', reset: 'Zurücksetzen',
    saveState: 'Zustand speichern', loadState: 'Zustand laden', share: 'Link teilen',
    registers: 'Register', flags: 'Flags', disasm: 'Disassemblierung', watch: 'Überwachen',
    memory: 'Speicher', memmap: 'Speicherkarte', ports: 'E/A-Ports',
    peripherals: 'Peripherie', output: 'Programmausgabe',
  },
  fr: {
    example: 'Exemple', assemble: 'Assembler', step: 'Pas', stepOver: 'Pas-sur',
    stepBack: 'Annuler', run: 'Lancer', stop: 'Arrêter', reset: 'Réinitialiser',
    saveState: 'Sauver état', loadState: 'Charger état', share: 'Partager',
    registers: 'Registres', flags: 'Drapeaux', disasm: 'Désassemblage', watch: 'Surveiller',
    memory: 'Mémoire', memmap: 'Carte mémoire', ports: 'Ports E/S',
    peripherals: 'Périphériques', output: 'Sortie du programme',
  },
  hi: {
    example: 'उदाहरण', assemble: 'असेंबल', step: 'चरण', stepOver: 'ओवर-चरण',
    stepBack: 'पीछे', run: 'चलाएँ', stop: 'रोकें', reset: 'रीसेट',
    saveState: 'स्थिति सहेजें', loadState: 'स्थिति लोड', share: 'लिंक साझा',
    registers: 'रजिस्टर', flags: 'फ्लैग', disasm: 'डिसअसेंबल', watch: 'वॉच',
    memory: 'मेमोरी', memmap: 'मेमोरी मानचित्र', ports: 'आई/ओ पोर्ट',
    peripherals: 'परिधीय', output: 'प्रोग्राम आउटपुट',
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
