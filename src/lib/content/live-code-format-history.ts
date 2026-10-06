import { StateEffect, StateField, type Transaction } from "@codemirror/state";
import { invertedEffects } from "@codemirror/commands";

export const formatIndentEffect = StateEffect.define<string | null>();
export const formatIndentField = StateField.define<string | null>({
  create: () => null,
  update(value, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(formatIndentEffect)) value = effect.value;
    }
    return value;
  },
});

export const formatPresentationHistory = [
  formatIndentField,
  invertedEffects.of((transaction) => transaction.effects.some((effect) => effect.is(formatIndentEffect))
    ? [formatIndentEffect.of(transaction.startState.field(formatIndentField))]
    : []),
];

export interface FormatPresentationChange {
  formatting: boolean;
  indentUnit?: string;
}

export function getFormatPresentationChange(transactions: readonly Transaction[]): FormatPresentationChange | undefined {
  let result: FormatPresentationChange | undefined;
  for (const transaction of transactions) {
    for (const effect of transaction.effects) {
      if (effect.is(formatIndentEffect)) {
        result = effect.value === null ? { formatting: false } : { formatting: true, indentUnit: effect.value };
      }
    }
  }
  return result;
}
