// EchoConnect components (window.EchoConnect). Types as documentation; props mirror components.js.
import * as React from "react";

type Glyph = string;
type Tone = "success" | "warning" | "danger" | "info" | "accent";

export interface IconProps { name: Glyph; size?: number; label?: string; strokeWidth?: number; }
export interface FileIconProps { name: string; size?: number; }
export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> { variant?: "primary" | "danger" | "ghost"; size?: "sm" | "lg"; icon?: Glyph; block?: boolean; }
export interface IconButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> { icon: Glyph; label: string; pressed?: boolean; size?: number; }
export interface SwitchProps { label: string; checked?: boolean; defaultChecked?: boolean; disabled?: boolean; onChange?: (on: boolean) => void; id?: string; }
export interface CheckboxProps { children?: React.ReactNode; checked?: boolean; defaultChecked?: boolean; onChange?: (on: boolean) => void; id?: string; }
export interface SegmentedControlProps { label: string; options: { value: string; text?: string; icon?: Glyph; label?: string }[]; value?: string; onChange?: (v: string) => void; block?: boolean; }
export interface TextFieldProps extends React.InputHTMLAttributes<HTMLInputElement> { id: string; label?: string; icon?: Glyph; hint?: string; error?: string; trailing?: React.ReactNode; }
export interface StatePillProps { children: React.ReactNode; tone?: Tone; icon?: Glyph; }
export interface SpinnerProps { large?: boolean; label?: string; }
export interface ProgressBarProps { value?: number; done?: boolean; }
export interface SnackbarProps { children: React.ReactNode; icon?: Glyph; tone?: "success" | "danger"; action?: string; }
export interface BannerProps { children?: React.ReactNode; title?: string; tone?: "warning" | "danger" | "success"; icon?: Glyph; action?: React.ReactNode; }
export interface AppBarProps { title: string; sub?: string; back?: boolean; actions?: { icon: Glyph; label: string; pressed?: boolean }[]; }
export interface BottomNavProps { active?: 0 | 1 | 2 | 3; badges?: Record<number, number>; onChange?: (i: number) => void; }
export interface SectionHeaderProps { children: React.ReactNode; action?: string; }
export interface ListGroupProps { title?: string; action?: string; foot?: React.ReactNode; children: React.ReactNode; }
export interface ListRowProps { title: React.ReactNode; sub?: React.ReactNode; icon?: Glyph; tint?: "ok"; file?: string; avatar?: string; avatarColor?: string; lead?: React.ReactNode; trailing?: React.ReactNode; chevron?: boolean; below?: React.ReactNode; disabled?: boolean; onClick?: () => void; }
export interface BottomSheetProps { title?: string; icon?: Glyph; children: React.ReactNode; footer?: React.ReactNode; }
export interface DialogProps { title: string; children: React.ReactNode; actions: React.ReactNode; tone?: "danger"; }
export interface PhoneFrameProps { children: React.ReactNode; width?: number; height?: number; time?: string; battery?: number; bluetooth?: boolean; dim?: boolean; }
export interface LaptopDeviceProps { width?: number; state?: "connected" | "locked" | "away"; }
export interface BatteryMeterProps { level: number; charging?: boolean; label?: boolean; }
export interface LinkPillsProps { wifi?: boolean; bluetooth?: boolean; network?: string; }
export interface LaptopCardProps { state?: "connected" | "away"; locked?: boolean; battery?: number; charging?: boolean; wifi?: boolean; bluetooth?: boolean; network?: string; deviceWidth?: number; }
export interface ActionGridProps { items?: { icon: Glyph; label: string; sub: string }[]; disabled?: boolean; }
export interface ClipItemProps { dir: "to" | "from"; text?: string; image?: number; when?: string; sensitive?: boolean; via?: "bt"; }
export interface ClipModeCardProps { mode: "auto" | "paused" | "manual"; }
export interface TransferRowProps { name: string; progress?: number; rate?: string; size?: string; dir?: "to" | "from"; when?: string; failed?: boolean; }
export interface PermissionRowProps { icon: Glyph; title: string; why: string; granted?: boolean; action?: string; optional?: boolean; }
export interface StepListProps { steps: { title: string; body?: React.ReactNode; done?: string }[]; current: number; }
export interface StepperProps { current: number; count?: number; label?: string; }
export interface QrViewfinderProps { found?: boolean; }
export interface PairCodeProps { code?: [string, string, string, string]; }
export interface CallBarProps { who?: string; time?: string; }
export interface SystemNotificationProps { title?: string; text?: string; actions?: string[]; }
export interface QuickTileProps { icon?: Glyph; label?: string; sub?: string; on?: boolean; }
export interface SystemDialogProps { title?: string; body?: string; actions?: string[]; }
export interface HomeScreenProps { state?: "connected" | "away" | "call"; locked?: boolean; }
export interface ClipboardScreenProps { mode?: "auto" | "paused" | "manual"; }
export interface ClipboardSetupScreenProps { step?: number; }
export interface PairingScreenProps { step?: number; found?: boolean; }
