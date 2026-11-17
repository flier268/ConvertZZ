import type source from "../zh-TW/dictionary";
import type { LocaleMessages } from "../types";

const messages: LocaleMessages<typeof source> = {
  title: "User dictionary",
  subtitle:
    "Custom whole-word mappings. Entries are added to segmentation and applied as whole words during conversion, never as plain text replacement.",
  import: "Import",
  export: "Export",
  add: "Add",
  deleteSelected: "Delete selected",
  deleteSelectedCount: "Delete selected ({count})",
  resetChanges: "Discard unsaved changes",
  info: "Simplified → Traditional uses the Simplified word as the source; Traditional → Simplified uses the Traditional word. Source words may contain only CJK characters. Import accepts .txt, .tsv and .csv and detects whether the file is a dictionary exported by this app or a 1.x legacy dictionary; imported entries are stored in the user dictionary, and the original file is never modified or read again.",
  loadError: "Could not read the user dictionary; converting with an empty dictionary: {error}",
  fileFilterName: "Dictionary files",
  defaultExportName: "ConvertZZ-user-dictionary.txt",
  direction: {
    both: "Both directions",
    s2t: "Simplified → Traditional only",
    t2s: "Traditional → Simplified only",
  },
  status: {
    importable: "Importable",
    disabled: "Disabled",
    invalid: "Invalid",
    duplicate: "Duplicate",
  },
  filter: {
    search: "Search Simplified or Traditional",
    directionLabel: "Filter by direction",
    allDirections: "All directions",
    sourceLabel: "Filter by source",
    allSources: "All sources",
    rowCount: "{visible} / {total} entries",
  },
  column: {
    selected: "Select",
    selectAllVisible: "Select all filtered entries",
    selectRow: "Select this row",
    simplified: "Simplified",
    traditional: "Traditional",
    direction: "Direction",
    source: "Import source",
    actions: "Actions",
    line: "Line",
    status: "Status",
    reason: "Reason",
  },
  manualSource: "Added manually",
  removal: {
    title: "Delete entries",
    emptyEntry: "empty entry",
    confirmOne:
      "Delete “{label}” from the table? Changes are written to the dictionary only after you click Save; until then, click “Discard unsaved changes” to undo.",
    confirmSelected:
      "Delete the selected entry from the table? Changes are written to the dictionary only after you click Save; until then, click “Discard unsaved changes” to undo. | Delete the {count} selected entries from the table? Changes are written to the dictionary only after you click Save; until then, click “Discard unsaved changes” to undo.",
    removed:
      "Deleted {count} entry from the table. Click Save to apply. | Deleted {count} entries from the table. Click Save to apply.",
  },
  conflict: "Source words map to different words in the same direction: {list}",
  listSeparator: ", ",
  save: {
    confirmTitle: "Save user dictionary",
    confirm:
      "{count} entry will be written and the conversion engine reloaded. Continue? | {count} entries will be written and the conversion engine reloaded. Continue?",
    saved: "Saved {count} entry. | Saved {count} entries.",
  },
  importDialog: {
    title: "Import dictionary",
    legacyTitle: "Importing a legacy dictionary is not recommended",
    legacyConfirm:
      "“{sourceFile}” is a 1.x legacy dictionary and importing it directly is not recommended: the built-in 1.x entries vary in quality, and 2.0 already converts some of those words better, so importing would override them. Consider adding only the words you added yourself by hand. Import anyway?",
    formatLegacy: "1.x legacy dictionary",
    formatUser: "User dictionary",
    summary:
      "{format}, {total} rows: {importable} importable, {disabled} disabled, {invalid} invalid, {duplicate} duplicate.",
    note: "Imported entries are tagged with the source “{sourceFile}” and the import time, merged into the current table and saved immediately.",
    unsavedWarning: "Unsaved changes in the table will also be saved.",
    confirm: "Import and save",
    conflict: "{conflict}. Fix the table before importing.",
    notSaved: "The imported entries were added to the table but have not been saved.",
    added: "Added {count} entry | Added {count} entries",
    existing:
      "{count} entry already in the table, now marked as from this import | {count} entries already in the table, now marked as from this import",
    conflicts:
      "{count} entry conflicts with an existing entry; kept the existing one | {count} entries conflict with existing entries; kept the existing ones",
    savedEdits: "unsaved changes in the table were saved as well",
    noteSeparator: "; ",
    done: "{notes}. Saved to the user dictionary.",
  },
  exported: "Exported {count} entry. | Exported {count} entries.",
  exportedWithUnsaved:
    "Exported {count} entry (including unsaved changes). | Exported {count} entries (including unsaved changes).",
};

export default messages;
