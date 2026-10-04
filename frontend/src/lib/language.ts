// The language messages to a person use (English or Spanish).

import { request } from "@/lib/api";

export type Language = "en" | "es";

export const LANGUAGE_WORDS: Record<Language, string> = {
  en: "English",
  es: "Español",
};

/** The browser's language when it's one we write in, else English. */
export function browserLanguage(): Language {
  if (typeof navigator === "undefined") return "en";
  return navigator.language?.toLowerCase().startsWith("es") ? "es" : "en";
}

interface LanguageResp {
  language: Language;
  addresses: number;
}

export const language = {
  lease: (id: string) =>
    request<LanguageResp>(`/leases/${id}/language`, { auth: true }),
  setLease: (id: string, lang: Language) =>
    request<LanguageResp>(`/leases/${id}/language`, {
      method: "PUT",
      auth: true,
      body: { language: lang },
    }),
  mine: () => request<LanguageResp>("/my/language", { auth: true }),
  setMine: (lang: Language) =>
    request<LanguageResp>("/my/language", {
      method: "PUT",
      auth: true,
      body: { language: lang },
    }),
};
