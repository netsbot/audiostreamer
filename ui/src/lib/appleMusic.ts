import { invoke } from "@tauri-apps/api/core";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { get, set } from "tauri-plugin-cache-api";

const DEV_TOKEN_CACHE_KEY = "audiostreamer:apple-music-dev-token:v1";
const USER_TOKEN_CACHE_KEY = "audiostreamer:apple-music-user-token:v1";

function readCachedValue(key: string): string | null {
  if (typeof localStorage === "undefined") return null;

  try {
    const value = localStorage.getItem(key);
    return value && value.trim() ? value : null;
  } catch {
    return null;
  }
}

function writeCachedValue(key: string, value: string) {
  if (typeof localStorage === "undefined") return;

  try {
    localStorage.setItem(key, value);
  } catch {
    // Ignore cache write failures.
  }
}

function clearCachedValue(key: string) {
  if (typeof localStorage === "undefined") return;

  try {
    localStorage.removeItem(key);
  } catch {
    // Ignore cache clear failures.
  }
}

async function loadToken(
  cacheKey: string,
  commandName: string,
  forceRefresh = false,
): Promise<string> {
  if (!forceRefresh) {
    const cached = readCachedValue(cacheKey);
    if (cached) {
      return cached;
    }
  }

  try {
    const token = (await invoke(commandName)) as string;
    if (!token.trim()) {
      throw new Error(`empty token from ${commandName}`);
    }

    writeCachedValue(cacheKey, token);
    return token;
  } catch (error) {
    clearCachedValue(cacheKey);
    throw error;
  }
}

export async function getAppleMusicDevToken(forceRefresh = false): Promise<string> {
  return loadToken(DEV_TOKEN_CACHE_KEY, "get_apple_music_token", forceRefresh);
}

export async function getAppleMusicUserToken(forceRefresh = false): Promise<string> {
  return loadToken(USER_TOKEN_CACHE_KEY, "get_apple_music_user_token", forceRefresh);
}

export async function fetchAppleMusic(
  url: string,
  init: RequestInit = {},
  options: { retryOnAuthFailure?: boolean; forceRefresh?: boolean } = {},
): Promise<Response> {
  const retryOnAuthFailure = options.retryOnAuthFailure ?? true;
  const forceRefresh = options.forceRefresh ?? false;

  const buildHeaders = async (forceRefreshTokens: boolean) => {
    const [devToken, userToken] = await Promise.all([
      getAppleMusicDevToken(forceRefreshTokens),
      getAppleMusicUserToken(forceRefreshTokens),
    ]);

    const headers = new Headers(init.headers ?? {});
    headers.set("Authorization", `Bearer ${devToken}`);
    headers.set("media-user-token", userToken);
    if (!headers.has("Origin")) {
      headers.set("Origin", "https://music.apple.com");
    }
    if (!headers.has("Referer")) {
      headers.set("Referer", "https://music.apple.com/");
    }

    return headers;
  };

  const request = async (forceRefreshTokens: boolean) => {
    const headers = await buildHeaders(forceRefreshTokens);

    return tauriFetch(url, {
      ...init,
      headers,
    });
  };

  let response = await request(forceRefresh);
  if (retryOnAuthFailure && (response.status === 401 || response.status === 403)) {
    clearCachedValue(DEV_TOKEN_CACHE_KEY);
    clearCachedValue(USER_TOKEN_CACHE_KEY);
    response = await request(true);
  }

  return response;
}

export async function fetchAppleMusicJson(
  url: string,
  cacheKey: string,
  ttl: number = 900,
  init: RequestInit = {},
  options: { retryOnAuthFailure?: boolean; forceRefresh?: boolean } = {},
): Promise<any> {
  if (!options.forceRefresh) {
    const cached = await get<any>(cacheKey);
    if (cached) {
      console.log(`[cache HIT] ${cacheKey}`, typeof cached, cached ? Object.keys(cached) : "null");
      return cached;
    }
  }

  console.log(`[cache MISS] ${cacheKey} — fetching network`);
  const response = await fetchAppleMusic(url, init, options);
  if (!response.ok) {
    throw new Error(`API failed: ${response.status}`);
  }
  
  const data = await response.json();
  await set(cacheKey, data, { ttl, compress: true });
  return data;
}