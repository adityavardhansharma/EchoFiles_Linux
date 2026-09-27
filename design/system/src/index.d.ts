import type * as React from 'react';

/** Glyph names shipped in assets/icons/glyph (86). */
export type GlyphName = string;
/** Colour icon names shipped in assets/icons/color (71). */
export type ColorIconName = string;
export type World = 'linux' | 'windows' | 'phone';
export type DriveState = 'mounted' | 'readonly' | 'dirty' | 'locked' | 'unmounted' | 'mounting';

export interface IconProps { name: GlyphName; size?: number; strokeWidth?: number; label?: string; className?: string }
export declare function Icon(props: IconProps): React.ReactElement;
export interface FileIconProps { name: ColorIconName; size?: number; badge?: 'link' | 'cloud' | 'lock' | 'broken' | 'ads'; className?: string }
export declare function FileIcon(props: FileIconProps): React.ReactElement;
/** Maps a filename (and kind) to a colour icon name. */
export declare function iconFor(name: string, kind?: 'folder' | 'file'): ColorIconName;

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> { variant?: 'primary' | 'danger' | 'ghost'; size?: 'sm'; icon?: GlyphName; kbd?: string[] }
export declare function Button(props: ButtonProps): React.ReactElement;
export interface IconButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> { icon: GlyphName; label: string; pressed?: boolean }
export declare function IconButton(props: IconButtonProps): React.ReactElement;
export interface SegmentOption { value: string; icon?: GlyphName; text?: string; label?: string; title?: string }
export interface SegmentedControlProps { options: SegmentOption[]; value?: string; onChange?: (v: string) => void; label?: string }
export declare function SegmentedControl(props: SegmentedControlProps): React.ReactElement;
export interface SwitchProps { id?: string; checked?: boolean; defaultChecked?: boolean; onChange?: (on: boolean) => void; label?: string; children?: React.ReactNode }
export declare function Switch(props: SwitchProps): React.ReactElement;
export interface CheckboxProps { id?: string; checked?: boolean | 'mixed'; defaultChecked?: boolean; onChange?: (on: boolean) => void; children?: React.ReactNode }
export declare function Checkbox(props: CheckboxProps): React.ReactElement;
export interface KbdProps { keys: string[] }
export declare function Kbd(props: KbdProps): React.ReactElement;

export interface TextFieldProps extends React.InputHTMLAttributes<HTMLInputElement> { label?: string; icon?: GlyphName; error?: string; trailing?: React.ReactNode }
export declare function TextField(props: TextFieldProps): React.ReactElement;
export interface SearchFieldProps { id?: string; placeholder?: string; defaultValue?: string; scope?: string; className?: string }
export declare function SearchField(props: SearchFieldProps): React.ReactElement;
export interface PathSegment { label: string; icon?: GlyphName }
export interface PathBarProps { segments?: PathSegment[]; editing?: boolean; path?: string; animateLast?: boolean; id?: string }
export declare function PathBar(props: PathBarProps): React.ReactElement;

export interface TabStripProps { tabs: { label: string; icon?: GlyphName }[]; active?: number }
export declare function TabStrip(props: TabStripProps): React.ReactElement;
export interface ToolbarProps { segments?: PathSegment[]; scope?: string; query?: string; grid?: boolean; viewOpen?: boolean; settings?: boolean }
export declare function ViewButton(props: { grid?: boolean; open?: boolean }): React.ReactElement;
export declare function ViewMenu(props: { grid?: boolean; dual?: boolean; preview?: boolean; hidden?: boolean }): React.ReactElement;
export declare function Toolbar(props: ToolbarProps): React.ReactElement;
export declare function Sidebar(props: { children?: React.ReactNode }): React.ReactElement;
export interface SidebarSectionProps { title: string; world?: World; count?: number; children?: React.ReactNode }
export declare function SidebarSection(props: SidebarSectionProps): React.ReactElement;
export interface SidebarItemProps { icon?: GlyphName; label: string; active?: boolean; trail?: string; dropTarget?: boolean }
export declare function SidebarItem(props: SidebarItemProps): React.ReactElement;
export interface DriveItemProps { name: string; icon?: GlyphName; state?: DriveState; used?: number; world?: World; meta?: string; free?: string; active?: boolean }
export declare function DriveItem(props: DriveItemProps): React.ReactElement;

export interface FileEntry {
  name: string; kind?: 'folder' | 'file'; icon?: ColorIconName; size?: string; items?: number; type?: string; modified?: string;
  selected?: boolean; cursor?: boolean; cut?: boolean; hidden?: boolean; drop?: boolean; removing?: boolean;
  badge?: FileIconProps['badge']; thumb?: string; sub?: string;
}
export interface FileRowProps { file: FileEntry; renaming?: boolean; onClick?: () => void }
export declare function FileRow(props: FileRowProps): React.ReactElement;
export interface ColumnHeaderProps { sort?: 'name' | 'size' | 'type' | 'modified'; desc?: boolean }
export declare function ColumnHeader(props: ColumnHeaderProps): React.ReactElement;
export interface FileListProps { files: FileEntry[]; sort?: ColumnHeaderProps['sort']; desc?: boolean; density?: 'compact' | 'default' | 'comfortable'; loading?: number; renaming?: number; cursor?: number; label?: string }
export declare function FileList(props: FileListProps): React.ReactElement;
export declare function Skeleton(props: { rows?: number }): React.ReactElement;
export declare function FileTile(props: { file: FileEntry }): React.ReactElement;
export declare function FileGrid(props: { files: FileEntry[]; label?: string }): React.ReactElement;
export interface StatusBarProps { count: string | number; selected?: number; selectedSize?: string; task?: string; volume?: string; state?: DriveState; free?: string }
export declare function StatusBar(props: StatusBarProps): React.ReactElement;
export interface EmptyStateProps { icon?: ColorIconName; title?: string; body?: string; actions?: React.ReactNode }
export declare function EmptyState(props: EmptyStateProps): React.ReactElement;

export interface DriveCardProps { name: string; icon?: ColorIconName; fs: string; used?: number; world?: World; free: string; total: string; state?: DriveState }
export declare function DriveCard(props: DriveCardProps): React.ReactElement;
export interface UsageBarProps { value: number; world?: World; color?: string; large?: boolean; label?: string }
export declare function UsageBar(props: UsageBarProps): React.ReactElement;
export interface StatePillProps { state?: DriveState | 'cloud' | 'ads' | 'hidden' | 'system'; tone?: 'success' | 'warning' | 'danger' | 'info' | 'accent' | null; children?: React.ReactNode }
export declare function StatePill(props: StatePillProps): React.ReactElement;

export declare function Spinner(props: { large?: boolean; label?: string }): React.ReactElement;
export interface BannerProps { tone?: 'info' | 'warning' | 'danger'; icon?: GlyphName; actions?: React.ReactNode; onClose?: false; children?: React.ReactNode }
export declare function Banner(props: BannerProps): React.ReactElement;
export interface ToastProps { title: string; tone?: 'success' | 'danger'; icon?: GlyphName; actions?: React.ReactNode; children?: React.ReactNode }
export declare function Toast(props: ToastProps): React.ReactElement;
export interface TransferToastProps { title: string; detail?: string; value?: number; indeterminate?: boolean; animate?: boolean; meta?: string; eta?: string; icon?: GlyphName; doneTitle?: string; doneMeta?: string }
export declare function TransferToast(props: TransferToastProps): React.ReactElement;
export declare function Tooltip(props: { kbd?: string[]; children?: React.ReactNode }): React.ReactElement;

export interface DialogProps { title: string; icon?: ColorIconName; tone?: 'danger'; footer?: React.ReactNode; height?: number; children?: React.ReactNode }
export declare function Dialog(props: DialogProps): React.ReactElement;
export declare function ConflictDialog(props: { title?: string; count?: number }): React.ReactElement;
export type MenuItem = '-' | { heading: string } | { icon?: GlyphName; label: string; kbd?: string[]; hint?: string; submenu?: boolean; danger?: boolean; disabled?: boolean; active?: boolean; checked?: boolean };
export declare function ContextMenu(props: { items: MenuItem[]; label?: string }): React.ReactElement;
export interface PaletteGroup { heading: string; items: { icon: GlyphName; label: string; hint?: string; kbd?: string[] }[] }
export declare function CommandPalette(props: { groups: PaletteGroup[]; query?: string; id?: string }): React.ReactElement;
export interface PreviewPaneProps { file: FileEntry; props?: [string, string][]; attrs?: string[]; windowsPath?: boolean }
export declare function PreviewPane(props: PreviewPaneProps): React.ReactElement;

export interface SettingsGroupProps { title: string; children?: React.ReactNode }
export declare function SettingsGroup(props: SettingsGroupProps): React.ReactElement;
export interface SettingRowProps { label: string; description?: string; disabled?: boolean; children?: React.ReactNode }
export declare function SettingRow(props: SettingRowProps): React.ReactElement;
export declare function SettingBlock(props: { muted?: boolean; children?: React.ReactNode }): React.ReactElement;
export interface PathListEditorProps { id?: string; paths?: string[]; icon?: GlyphName; empty?: string; placeholder?: string; current?: string; error?: string; note?: (string | null)[]; draft?: string }
export declare function PathListEditor(props: PathListEditorProps): React.ReactElement;
export declare function NameChips(props: { id?: string; names?: string[] }): React.ReactElement;
export type IndexState = 'off' | 'opening' | 'building' | 'updating' | 'ready' | 'problem';
export declare function IndexStatus(props: { state?: IndexState; detail?: string }): React.ReactElement;
export declare function CommandList(props: { items: [string, string][] }): React.ReactElement;
export type SettingsPageId = 'general' | 'search' | 'agents' | 'appearance' | 'about';
export declare function SettingsNav(props: { page: SettingsPageId; onChange?: (p: SettingsPageId) => void }): React.ReactElement;
export declare function SettingsShell(props: { page: SettingsPageId; onPage?: (p: SettingsPageId) => void; saved?: boolean; height?: number; children?: React.ReactNode }): React.ReactElement;
export declare function SettingsPage(props: { height?: number; saved?: boolean; page?: SettingsPageId }): React.ReactElement;
export interface SearchHit { name: string; kind: 'file' | 'folder'; location: string; size: string; date: string }
export declare function SearchResults(props: { hits?: SearchHit[] }): React.ReactElement;
export declare function SearchScope(props: { value?: 'folder' | 'everywhere'; onChange?: (v: 'folder' | 'everywhere') => void }): React.ReactElement;

export declare function AppWindow(props: { height?: number; banner?: boolean; animate?: boolean }): React.ReactElement;
export declare function DualPane(props: { height?: number }): React.ReactElement;
export declare function MotionSpec(props: {}): React.ReactElement;

declare global { interface Window { Echo: typeof import('./index') } }
