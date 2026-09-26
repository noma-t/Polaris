import type { ComponentType } from "react";
import { InfoIcon, InstancesIcon, MoonIcon, SettingsIcon } from "./icons";

export type Screen = "login" | "instances" | "recommend" | "settings" | "info";
export type NavScreen = Exclude<Screen, "login">;

export const NAV_SCREENS: readonly NavScreen[] = ["instances", "recommend", "settings", "info"];

export const NAV_ITEMS: { screen: NavScreen; label: string; Icon: ComponentType<{ size?: number }> }[] = [
  { screen: "instances", label: "Instances", Icon: InstancesIcon },
  { screen: "recommend", label: "V-Sui Groups", Icon: MoonIcon },
  { screen: "settings", label: "Settings", Icon: SettingsIcon },
  { screen: "info", label: "About", Icon: InfoIcon },
];
