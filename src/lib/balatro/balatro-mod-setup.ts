import { invoke } from "@tauri-apps/api/core";

export interface AutoDetectedBalatroPaths {
  appdataPath: string | null;
  gamePath: string | null;
}

export interface BalatroSetupResult {
  appdataPath: string;
  gamePath: string;
  modsPath: string;
}

export interface BalatroModSetupOptions {
  appdataPath?: string;
  gamePath?: string;
}

export interface BalatroModSetupStatus {
  platform: string;
  steamoddedInstalled: boolean;
  lovelyInstalled: boolean;
  canInstall: boolean;
  appdataPath: string | null;
  gamePath: string | null;
  modsPath: string | null;
  issues: string[];
  steamoddedVersion?: string | null;
  lovelyVersion?: string | null;
}

export interface BalatroModSetupInstallResult {
  status: BalatroModSetupStatus;
  steamoddedVersion: string;
  lovelyVersion: string;
  backups: string[];
}

const modSetupArgs = (options?: BalatroModSetupOptions) => ({
  appdataPath: options?.appdataPath?.trim() || null,
  gamePath: options?.gamePath?.trim() || null,
});

export const inspectBalatroModSetup = async (
  options?: BalatroModSetupOptions,
): Promise<BalatroModSetupStatus> =>
  invoke<BalatroModSetupStatus>("inspect_balatro_mod_setup", modSetupArgs(options));

export const installLatestBalatroModSetup = async (
  options?: BalatroModSetupOptions,
): Promise<BalatroModSetupInstallResult> =>
  invoke<BalatroModSetupInstallResult>("install_latest_balatro_mod_setup", modSetupArgs(options));

export const autoFindBalatroPaths = async (options?: {
  configuredAppdataPath?: string;
  configuredGamePath?: string;
  legacyPath?: string;
}): Promise<AutoDetectedBalatroPaths> => {
  return invoke<AutoDetectedBalatroPaths>("auto_find_balatro_paths", {
    configuredAppdataPath: options?.configuredAppdataPath ?? null,
    configuredGamePath: options?.configuredGamePath ?? null,
    legacyPath: options?.legacyPath ?? null,
  });
};

export const ensureBalatroModSetup = async (options?: {
  appdataPath?: string;
  gamePath?: string;
  legacyPath?: string;
}): Promise<BalatroSetupResult> => {
  return invoke<BalatroSetupResult>("ensure_balatro_mod_setup", {
    appdataPath: options?.appdataPath ?? null,
    gamePath: options?.gamePath ?? null,
    legacyPath: options?.legacyPath ?? null,
  });
};
