import { useCallback, useReducer, type SetStateAction } from "react";
import type { Rule } from "./types";

const RULE_HISTORY_LIMIT = 64;

type RuleHistory = {
  past: Rule[][];
  rules: Rule[];
  future: Rule[][];
};

type RuleHistoryAction =
  | { type: "reset"; rules: Rule[] }
  | { type: "update"; update: SetStateAction<Rule[]> }
  | { type: "undo" }
  | { type: "redo" }
  | { type: "restore"; index: number };

const cloneRules = (rules: Rule[]): Rule[] =>
  JSON.parse(JSON.stringify(rules)) as Rule[];

// Keep blocks and their history in one state update. Loading or restoring rules
// must never be mistaken for an edit by an effect from an earlier render.
const reduceRuleHistory = (
  state: RuleHistory,
  action: RuleHistoryAction,
): RuleHistory => {
  switch (action.type) {
    case "reset":
      return { past: [], rules: cloneRules(action.rules), future: [] };
    case "update": {
      const rules = typeof action.update === "function"
        ? action.update(state.rules)
        : action.update;
      if (rules === state.rules || JSON.stringify(rules) === JSON.stringify(state.rules)) {
        return state;
      }
      return {
        past: [...state.past, cloneRules(state.rules)].slice(-RULE_HISTORY_LIMIT),
        rules: cloneRules(rules),
        future: [],
      };
    }
    case "undo": {
      if (state.past.length === 0) return state;
      return {
        past: state.past.slice(0, -1),
        rules: cloneRules(state.past[state.past.length - 1]),
        future: [...state.future, cloneRules(state.rules)].slice(-RULE_HISTORY_LIMIT),
      };
    }
    case "redo": {
      if (state.future.length === 0) return state;
      return {
        past: [...state.past, cloneRules(state.rules)].slice(-RULE_HISTORY_LIMIT),
        rules: cloneRules(state.future[state.future.length - 1]),
        future: state.future.slice(0, -1),
      };
    }
    case "restore": {
      const timeline = [...state.past, state.rules, ...state.future.slice().reverse()];
      if (!Number.isInteger(action.index) || action.index < 0
        || action.index >= timeline.length || action.index === state.past.length) {
        return state;
      }
      return {
        past: timeline.slice(0, action.index).slice(-RULE_HISTORY_LIMIT).map(cloneRules),
        rules: cloneRules(timeline[action.index]),
        future: timeline.slice(action.index + 1).reverse().slice(-RULE_HISTORY_LIMIT).map(cloneRules),
      };
    }
  }
};

export const useRuleHistory = () => {
  const [history, dispatch] = useReducer(reduceRuleHistory, {
    past: [], rules: [], future: [],
  });
  const setRules = useCallback((update: SetStateAction<Rule[]>) => {
    dispatch({ type: "update", update });
  }, []);
  const resetHistory = useCallback((rules: Rule[]) => {
    dispatch({ type: "reset", rules });
  }, []);
  const handleUndo = useCallback(() => dispatch({ type: "undo" }), []);
  const handleRedo = useCallback(() => dispatch({ type: "redo" }), []);
  const restoreHistoryAt = useCallback((index: number) => {
    dispatch({ type: "restore", index });
  }, []);

  return {
    rules: history.rules,
    setRules,
    resetHistory,
    handleUndo,
    handleRedo,
    restoreHistoryAt,
    historyTimeline: [...history.past, history.rules, ...history.future.slice().reverse()],
    historyCurrentIndex: history.past.length,
    canUndo: history.past.length > 0,
    canRedo: history.future.length > 0,
  };
};
