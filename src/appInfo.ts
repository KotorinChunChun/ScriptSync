import releaseVersionJson from '../dev/msix-version.json?raw';

const releaseVersion = JSON.parse(releaseVersionJson) as {applicationVersion:string;revision:number};
export const appInfo = {
  name: '読みあわせ (ScriptSync)',
  version: `${releaseVersion.applicationVersion}.${releaseVersion.revision}`,
  updatedAt: '2026-10-24',
} as const;
