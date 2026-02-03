export type ActionType = "Click" | "Shortcut" | "Note";

export interface Step {
  id: string;
  ts: number;
  action: ActionType;
  x: number;
  y: number;
  click_x_percent: number;
  click_y_percent: number;
  app: string;
  window_title: string;
  screenshot_path: string | null;
  note: string | null;
}
