type Modifiers = Map<string, string | null>;

const ALTERNATIVE_COLOUR_SOURCE: Record<string, string> = {
  C: "V", V: "C", X: "B", B: "X",
};

const readModifiers = (tag: string): Modifiers => {
  const modifiers: Modifiers = new Map();
  for (const modifier of tag.slice(1, -1).split(",")) {
    const colon = modifier.indexOf(":");
    if (colon > 0) {
      modifiers.set(modifier.slice(0, colon), modifier.slice(colon + 1));
    } else if (modifier) {
      modifiers.set(modifier, null);
    }
  }
  return modifiers;
};

const writeModifiers = (modifiers: Modifiers): string =>
  `{${Array.from(modifiers, ([key, value]) => value === null ? key : `${key}:${value}`).join(",")}}`;

const activeModifiers = (text: string): Modifiers => {
  const tags = text.match(/\{[^}]*\}/g);
  return tags?.length ? readModifiers(tags[tags.length - 1]) : new Map();
};

// apply toolbar formatting to every selected line without changing the suffix
export const insertDescriptionTag = (
  value: string,
  start: number,
  end: number,
  tag: string,
  autoClose = true,
): { value: string; cursor: number } => {
  const prefix = value.slice(0, start);
  const selected = value.slice(start, end);
  const suffix = value.slice(end);

  if (!autoClose) {
    const inserted = tag + selected;
    return { value: prefix + inserted + suffix, cursor: start + inserted.length };
  }

  let original = activeModifiers(prefix);
  const applied = readModifiers(tag);
  const styledTag = (): string => {
    const combined = new Map(original);
    for (const [key, value] of applied) {
      const alternative = ALTERNATIVE_COLOUR_SOURCE[key];
      if (alternative) combined.delete(alternative);
      combined.set(key, value);
    }
    return writeModifiers(combined);
  };
  if (!selected) {
    const openingTag = styledTag();
    return {
      value: prefix + openingTag + writeModifiers(original) + suffix,
      cursor: start + openingTag.length,
    };
  }

  const parts = selected.split(/(\{[^}]*\}|\[s\]|\r\n|\n|\r|<br\s*\/?>)/gi);
  let formatted = "";
  let lineHasStyle = false;

  for (const part of parts) {
    if (!part) continue;
    if (/^\{[^}]*\}$/.test(part)) {
      original = readModifiers(part);
      if (lineHasStyle) formatted += styledTag();
    } else if (/^(?:\[s\]|\r\n|\n|\r|<br\s*\/?>)$/i.test(part)) {
      if (lineHasStyle) formatted += "{}";
      formatted += part;
      lineHasStyle = false;
    } else {
      if (!lineHasStyle && /^\s*$/.test(part)) {
        formatted += part;
        continue;
      }
      if (!lineHasStyle) formatted += styledTag();
      formatted += part;
      lineHasStyle = true;
    }
  }

  formatted += writeModifiers(original);
  return { value: prefix + formatted + suffix, cursor: start + formatted.length };
};
