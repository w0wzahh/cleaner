const CLEANER_CATEGORIES = [
  {
    id: "user-temp",
    title: "User Temp Files",
    description: "App leftovers and temporary files in your user profile.",
    whyItHelps: "Temp folders can grow quickly and waste SSD/HDD space.",
    risk: "low",
    advanced: false,
    safeDefault: true,
    operations: [
      {
        type: "directoryContents",
        path: "%TEMP%",
      },
    ],
  },
  {
    id: "windows-temp",
    title: "Windows Temp Folder",
    description: "Temporary operating system files that are usually safe to clear.",
    whyItHelps: "Removes stale system temp data and installer leftovers.",
    risk: "low",
    advanced: false,
    safeDefault: true,
    operations: [
      {
        type: "directoryContents",
        path: "%WINDIR%\\Temp",
      },
    ],
  },
  {
    id: "recycle-bin",
    title: "Recycle Bin",
    description: "Empties existing Recycle Bin contents to reclaim space.",
    whyItHelps: "Large deleted files remain on disk until bin is emptied.",
    risk: "low",
    advanced: false,
    safeDefault: true,
    operations: [
      {
        type: "recycleBin",
      },
    ],
  },
  {
    id: "thumbnail-cache",
    title: "Thumbnail and Icon Cache",
    description: "Removes cached Explorer thumbnails and icon databases.",
    whyItHelps: "Cache can become bloated or corrupted over time.",
    risk: "low",
    advanced: false,
    safeDefault: true,
    operations: [
      {
        type: "directoryFilesByPattern",
        path: "%LOCALAPPDATA%\\Microsoft\\Windows\\Explorer",
        pattern: "^(thumbcache|iconcache).*\\.db$",
      },
    ],
  },
  {
    id: "delivery-optimization-cache",
    title: "Delivery Optimization Cache",
    description: "Clears Windows update delivery cache files.",
    whyItHelps: "Microsoft recommends Disk Cleanup for this category when disk space is needed.",
    risk: "low",
    advanced: false,
    safeDefault: true,
    operations: [
      {
        type: "directoryContents",
        path: "%ProgramData%\\Microsoft\\Windows\\DeliveryOptimization\\Cache",
      },
    ],
  },
  {
    id: "browser-caches",
    title: "Edge and Chrome Caches",
    description: "Cleans browser cache folders (cookies/passwords are not touched).",
    whyItHelps: "Browsers cache large amounts of web content over time.",
    risk: "low",
    advanced: false,
    safeDefault: true,
    operations: [
      {
        type: "directoryContents",
        path: "%LOCALAPPDATA%\\Microsoft\\Edge\\User Data\\Default\\Cache",
      },
      {
        type: "directoryContents",
        path: "%LOCALAPPDATA%\\Microsoft\\Edge\\User Data\\Default\\Code Cache",
      },
      {
        type: "directoryContents",
        path: "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Cache",
      },
      {
        type: "directoryContents",
        path: "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Code Cache",
      },
    ],
  },
  {
    id: "firefox-cache",
    title: "Firefox Cache",
    description: "Cleans Firefox disk cache for all local profiles.",
    whyItHelps: "Browser cache grows over time and can take significant space.",
    risk: "low",
    advanced: false,
    safeDefault: true,
    operations: [
      {
        type: "profileSubdirectories",
        path: "%LOCALAPPDATA%\\Mozilla\\Firefox\\Profiles",
        childDirectories: ["cache2", "startupCache", "jumpListCache"],
      },
    ],
  },
  {
    id: "directx-shader-cache",
    title: "DirectX Shader Cache",
    description: "Deletes compiled shader cache. Games/apps rebuild this automatically.",
    whyItHelps: "Shader cache can consume large space and occasionally become stale.",
    risk: "low",
    advanced: false,
    safeDefault: true,
    operations: [
      {
        type: "directoryContents",
        path: "%LOCALAPPDATA%\\D3DSCache",
      },
      {
        type: "directoryContents",
        path: "%LOCALAPPDATA%\\NVIDIA\\DXCache",
      },
      {
        type: "directoryContents",
        path: "%LOCALAPPDATA%\\NVIDIA\\GLCache",
      },
    ],
  },
  {
    id: "windows-error-reporting",
    title: "Windows Error Reporting Queue",
    description: "Removes queued and archived crash reports.",
    whyItHelps: "Error report archives can accumulate in the background.",
    risk: "low",
    advanced: false,
    safeDefault: true,
    operations: [
      {
        type: "directoryContents",
        path: "%ProgramData%\\Microsoft\\Windows\\WER\\ReportArchive",
      },
      {
        type: "directoryContents",
        path: "%ProgramData%\\Microsoft\\Windows\\WER\\ReportQueue",
      },
    ],
  },
  {
    id: "windows-log-files",
    title: "Windows Setup and Servicing Logs",
    description: "Cleans old CBS, DISM, and setup troubleshooting logs.",
    whyItHelps: "Log files can silently grow and occupy notable disk space.",
    risk: "medium",
    advanced: false,
    safeDefault: false,
    operations: [
      {
        type: "directoryContents",
        path: "%WINDIR%\\Logs\\CBS",
      },
      {
        type: "directoryContents",
        path: "%WINDIR%\\Logs\\DISM",
      },
      {
        type: "directoryContents",
        path: "%WINDIR%\\Panther",
      },
    ],
  },
  {
    id: "discord-and-teams-cache",
    title: "Discord and Teams Cache",
    description: "Cleans temporary cache files created by chat clients.",
    whyItHelps: "Collaboration apps frequently keep large temporary media caches.",
    risk: "low",
    advanced: false,
    safeDefault: true,
    operations: [
      {
        type: "directoryContents",
        path: "%APPDATA%\\discord\\Cache",
      },
      {
        type: "directoryContents",
        path: "%APPDATA%\\discord\\Code Cache",
      },
      {
        type: "directoryContents",
        path: "%APPDATA%\\Microsoft\\Teams\\Cache",
      },
      {
        type: "directoryContents",
        path: "%APPDATA%\\Microsoft\\Teams\\Code Cache",
      },
    ],
  },
  {
    id: "crash-dumps-and-logs",
    title: "Crash Dumps and Error Logs",
    description: "Deletes old dump files used for post-crash debugging.",
    whyItHelps: "Dump files are often very large and rarely needed by normal users.",
    risk: "medium",
    advanced: false,
    safeDefault: true,
    operations: [
      {
        type: "directoryContents",
        path: "%LOCALAPPDATA%\\CrashDumps",
      },
      {
        type: "directoryContents",
        path: "%WINDIR%\\Minidump",
      },
      {
        type: "file",
        path: "%WINDIR%\\MEMORY.DMP",
      },
    ],
  },
  {
    id: "windows-update-downloads",
    title: "Windows Update Download Cache",
    description: "Clears downloaded update packages that are already installed or stale.",
    whyItHelps: "Update caches can occupy several GB.",
    risk: "medium",
    advanced: false,
    safeDefault: false,
    operations: [
      {
        type: "directoryContents",
        path: "%WINDIR%\\SoftwareDistribution\\Download",
      },
    ],
  },
  {
    id: "prefetch-cache",
    title: "Prefetch Cache",
    description: "Removes prefetch traces. Windows will rebuild them automatically.",
    whyItHelps: "Can remove stale prefetch entries and free a small amount of space.",
    risk: "medium",
    advanced: false,
    safeDefault: false,
    operations: [
      {
        type: "directoryContents",
        path: "%WINDIR%\\Prefetch",
      },
    ],
  },
  {
    id: "hibernation-file",
    title: "Hibernation File (hiberfil.sys)",
    description: "Disables hibernation and removes hiberfil.sys to recover large disk space.",
    whyItHelps: "Can reclaim multiple GB on systems where hibernation is not used.",
    risk: "high",
    advanced: true,
    safeDefault: false,
    operations: [
      {
        type: "file",
        path: "%SystemDrive%\\hiberfil.sys",
      },
      {
        type: "command",
        shell: "cmd",
        command: "powercfg /hibernate off",
        note: "This disables hibernation. Re-enable with powercfg /hibernate on.",
      },
    ],
  },
  {
    id: "pagefile-optimize",
    title: "Pagefile Optimization (Advanced)",
    description: "Configures virtual memory to system-managed mode instead of hard disabling it.",
    whyItHelps: "Can reduce oversized custom pagefiles while keeping Windows stable.",
    risk: "high",
    advanced: true,
    safeDefault: false,
    operations: [
      {
        type: "file",
        path: "%SystemDrive%\\pagefile.sys",
      },
      {
        type: "command",
        shell: "cmd",
        command: "wmic computersystem where name=\"%computername%\" set AutomaticManagedPagefile=True",
        note: "Restart may be required for full effect.",
      },
    ],
  },
];

module.exports = {
  CLEANER_CATEGORIES,
};
