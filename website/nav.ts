// The site's navigation: sections and their pages, in reading order. A page is
// a path under docs/<lang>/, without `.md`; English and French share the list.

export type Lang = "en" | "fr";

export interface Section {
  title: Record<Lang, string>;
  pages: string[];
}

export const sections: Section[] = [
  {
    title: { en: "Guide", fr: "Guide" },
    pages: [
      "guide/introduction",
      "guide/getting-started",
      "guide/basics",
      "guide/functions",
      "guide/control-flow",
      "guide/structures",
      "guide/enums",
      "guide/generics",
      "guide/methods",
      "guide/collections",
      "guide/memory",
      "guide/diagnostics",
      "guide/javascript",
    ],
  },
  {
    title: { en: "Reference", fr: "Référence" },
    pages: [
      "reference/status",
      "reference/types",
      "reference/operators",
      "reference/methods",
      "reference/keywords",
      "reference/cli",
      "reference/roadmap",
    ],
  },
  {
    title: { en: "Errors", fr: "Erreurs" },
    pages: ["errors/index"],
  },
  {
    title: { en: "Project", fr: "Projet" },
    pages: [
      "spec",
      "architecture",
      "kickoff-plan",
      "decisions/0001-webassembly-backend",
      "decisions/0002-tlsf-allocator",
      "decisions/0003-runtime-proved-at-compile-time",
    ],
  },
];

/** Words of the interface. */
export interface Words {
  search: string;
  searchPlaceholder: string;
  noResults: string;
  onThisPage: string;
  previous: string;
  next: string;
  edit: string;
  theme: string;
  menu: string;
  other: string;
  copy: string;
  copied: string;
  compiled: string;
  checked: string;
  planned: string;
  error: string;
}

/** Words of the interface, in each language. */
export const words: Record<Lang, Words> = {
  en: {
    search: "Search",
    searchPlaceholder: "Search the documentation",
    noResults: "No results",
    onThisPage: "On this page",
    previous: "Previous",
    next: "Next",
    edit: "Edit this page on GitHub",
    theme: "Switch between light and dark",
    menu: "Menu",
    other: "Français",
    copy: "Copy",
    copied: "Copied",
    compiled: "compiled to WebAssembly",
    checked: "checked, not compiled yet",
    planned: "planned: not in the language yet",
    error: "refused with",
  },
  fr: {
    search: "Rechercher",
    searchPlaceholder: "Rechercher dans la documentation",
    noResults: "Aucun résultat",
    onThisPage: "Sur cette page",
    previous: "Précédent",
    next: "Suivant",
    edit: "Modifier cette page sur GitHub",
    theme: "Passer du clair au sombre",
    menu: "Menu",
    other: "English",
    copy: "Copier",
    copied: "Copié",
    compiled: "compilé en WebAssembly",
    checked: "vérifié, pas encore compilé",
    planned: "prévu : pas encore dans le langage",
    error: "refusé avec",
  },
};
