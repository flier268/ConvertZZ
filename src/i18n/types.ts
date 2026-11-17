/** 讓英文訊息與繁中來源同形：鍵必須一一對應，值一律為字串。 */
export type LocaleMessages<T> = {
  [K in keyof T]: T[K] extends string ? string : LocaleMessages<T[K]>;
};

type Join<P extends string, K extends string> = P extends "" ? K : `${P}.${K}`;

export type MessagePath<T, P extends string = ""> = {
  [K in keyof T & string]: T[K] extends string ? Join<P, K> : MessagePath<T[K], Join<P, K>>;
}[keyof T & string];
