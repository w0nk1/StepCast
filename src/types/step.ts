export type ActionType = "Click" | "DoubleClick" | "RightClick" | "Shortcut" | "Note";

export type CaptureStatus = "Ok" | "Fallback" | "Failed";

export type DescriptionSource = "ai" | "manual";

export type DescriptionStatus = "idle" | "generating" | "failed";

export type BoundsPercent = {
  x_percent: number;
  y_percent: number;
  width_percent: number;
  height_percent: number;
};

export type AxClickInfo = {
  role: string;
  subrole?: string | null;
  role_description?: string | null;
  identifier?: string | null;
  label: string;
  element_bounds?: BoundsPercent | null;
  container_role?: string | null;
  container_subrole?: string | null;
  container_identifier?: string | null;
  window_role?: string | null;
  window_subrole?: string | null;
  top_level_role?: string | null;
  top_level_subrole?: string | null;
  parent_dialog_role?: string | null;
  parent_dialog_subrole?: string | null;
  is_checked?: boolean | null;
  is_cancel_button: boolean;
  is_default_button: boolean;
};

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
  description?: string | null;
  description_source?: DescriptionSource | null;
  description_status?: DescriptionStatus | null;
  description_error?: string | null;
  ax?: AxClickInfo | null;
  capture_status?: CaptureStatus | null;
  capture_error?: string | null;
  crop_region?: BoundsPercent | null;
}
