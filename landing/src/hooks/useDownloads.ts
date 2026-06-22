import { useEffect, useState } from "react";

/**
 * Resolves the real per-OS download assets from the even-releases GitHub repo.
 * Ships with the known-good v0.0.3-beta URLs as a fallback, then fetches the
 * latest release at runtime so the buttons always point at the newest build
 * (GitHub's API sends permissive CORS headers, so this works client-side).
 */
export type OSKey = "mac" | "win" | "deb" | "appimage";

const REPO = "leox255/even-releases";
const FALLBACK_TAG = "v0.0.3-beta";
const asset = (name: string) =>
  `https://github.com/${REPO}/releases/download/${FALLBACK_TAG}/${name}`;

const FALLBACK: Record<OSKey, string> = {
  mac: asset("Even_0.0.3-beta_universal.dmg"),
  win: asset("Even_0.0.3-beta_x64-setup.exe"),
  deb: asset("Even_0.0.3-beta_amd64.deb"),
  appimage: asset("Even_0.0.3-beta_amd64.AppImage"),
};

const MATCH: Record<OSKey, RegExp> = {
  mac: /\.dmg$/i,
  win: /-setup\.exe$/i,
  deb: /\.deb$/i,
  appimage: /\.AppImage$/i,
};

export const RELEASES_PAGE = `https://github.com/${REPO}/releases/latest`;

export function detectOS(): OSKey {
  if (typeof navigator === "undefined") return "mac";
  const ua = navigator.userAgent;
  if (/Windows|Win64|Win32/i.test(ua)) return "win";
  if (/Mac OS X|Macintosh/i.test(ua)) return "mac";
  if (/Linux|X11/i.test(ua) && !/Android/i.test(ua)) return "appimage";
  return "mac";
}

type Release = {
  tag_name?: string;
  assets?: { name: string; browser_download_url: string }[];
};

export function useDownloads() {
  const [urls, setUrls] = useState<Record<OSKey, string>>(FALLBACK);
  const [version, setVersion] = useState(FALLBACK_TAG);
  const [primary] = useState<OSKey>(() => detectOS());

  useEffect(() => {
    let alive = true;
    fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
      headers: { Accept: "application/vnd.github+json" },
    })
      .then((r) => (r.ok ? (r.json() as Promise<Release>) : Promise.reject(r.status)))
      .then((data) => {
        if (!alive || !data?.assets?.length) return;
        const next = { ...FALLBACK };
        (Object.keys(MATCH) as OSKey[]).forEach((k) => {
          const hit = data.assets!.find((a) => MATCH[k].test(a.name) && !/\.sig$/i.test(a.name));
          if (hit) next[k] = hit.browser_download_url;
        });
        setUrls(next);
        if (data.tag_name) setVersion(data.tag_name);
      })
      .catch(() => {
        /* keep fallback URLs */
      });
    return () => {
      alive = false;
    };
  }, []);

  return { urls, version, primary, page: RELEASES_PAGE };
}

export const OS_BUTTON_LABEL: Record<OSKey, string> = {
  mac: "macOS",
  win: "Windows",
  deb: "Linux",
  appimage: "Linux",
};
