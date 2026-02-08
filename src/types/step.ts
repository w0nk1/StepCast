export type ActionType = "Click" | "DoubleClick" | "RightClick" | "Shortcut" | "Note";

export type CaptureStatus = "Ok" | "Fallback" | "Failed";

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
  capture_status?: CaptureStatus | null;
  capture_error?: string | null;
}
