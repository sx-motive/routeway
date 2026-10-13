/**
 * Russian plural forms: 1 домен, 2 домена, 5 доменов.
 * Takes [one, few, many] and picks by the usual 1 / 2-4 / 0,5-20 rule.
 */
export function plural(count: number, forms: [string, string, string]): string {
  const n = Math.abs(count) % 100;
  const n1 = n % 10;
  if (n > 10 && n < 20) return forms[2];
  if (n1 > 1 && n1 < 5) return forms[1];
  if (n1 === 1) return forms[0];
  return forms[2];
}

export const countOf = (count: number, forms: [string, string, string]) =>
  `${count} ${plural(count, forms)}`;
