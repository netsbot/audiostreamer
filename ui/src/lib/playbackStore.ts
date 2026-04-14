import { writable } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { fetch as tauriFetch } from '@tauri-apps/plugin-http';

export interface TrackMetadata {
    title: string;
    artist: string;
    album: string;
    artwork_url?: string;
    duration_ms?: number;
}

export interface StationMetadata {
    name: string;
    subtitle?: string;
    artwork_url?: string;
}

export const currentTrack = writable<TrackMetadata | null>(null);
export const isPlaying = writable(false);
export const currentTime = writable(0); // in seconds
export const totalTime = writable(0);   // in seconds

type StationTrack = {
    id: string;
    attributes?: {
        name?: string;
        artistName?: string;
        albumName?: string;
        artwork?: any;
        durationInMillis?: number;
    };
};

function formatArtworkUrl(artwork: any, size = 600): string | undefined {
    if (!artwork?.url) return undefined;
    return artwork.url
        .replace('{w}', `${size}`)
        .replace('{h}', `${size}`)
        .replace('{f}', 'webp')
        .replace('{c}', '');
}

async function fetchStationFirstTrack(stationId: string): Promise<StationTrack | null> {
    const devToken = await invoke<string>('get_apple_music_token');
    const userToken = await invoke<string>('get_apple_music_user_token');

    const headers: Record<string, string> = {
        Authorization: `Bearer ${devToken}`,
        'media-user-token': userToken,
        Origin: 'https://music.apple.com',
        Referer: 'https://music.apple.com/',
        'Content-Type': 'application/json',
    };

    // Use POST next-tracks to get actual song data for stations
    const url = `https://api.music.apple.com/v1/me/stations/next-tracks/${stationId}?limit=1`;

    try {
        const response = await tauriFetch(url, { method: 'POST', headers });
        if (response.ok) {
            const payload: any = await response.json();
            const tracks = payload?.data || [];
            const first = Array.isArray(tracks) ? tracks[0] : null;
            if (first?.id) {
                return first as StationTrack;
            }
        }
    } catch (e) {
        console.warn('Failed to fetch station next-track:', e);
    }

    return null;
}

// Initialize listeners for the bridge
export async function initPlaybackBridge() {
    console.log("Initializing playback bridge...");

    // Listen for toggle from system media controls
    await listen('playback-toggle', async () => {
        console.log("System requested playback toggle");
        try {
            const playing = await invoke<boolean>('toggle_playback');
            isPlaying.set(playing);
        } catch (e) {
            console.error("Failed to toggle playback via bridge:", e);
        }
    });

    // Listen for progress updates from backend
    await listen('playback-progress', (event: any) => {
        const payload = event.payload as {
            currentTime?: number;
            current_time?: number;
            totalTime?: number;
            total_time?: number;
            paused?: boolean;
        };
        const current = payload.currentTime ?? payload.current_time ?? 0;
        const total = payload.totalTime ?? payload.total_time ?? 0;
        const paused = payload.paused ?? false;
        console.log("Playback progress received:", payload);
        console.debug("Playback progress:", current, "/", total);
        currentTime.set(current);
        if (total > 0) {
            totalTime.set(total);
        }
        isPlaying.set(!paused);

    });
}

export async function playSong(id: string, metadata: TrackMetadata) {
    console.log("Playing song:", id, metadata);
    try {
        // Optimistic UI
        currentTrack.set(metadata);
        isPlaying.set(true);
        currentTime.set(0);
        totalTime.set((metadata.duration_ms || 0) / 1000);

        await invoke('play_song', {
            request: {
                adamId: id,
                metadata: {
                    title: metadata.title,
                    artist: metadata.artist,
                    album: metadata.album,
                    artwork_url: metadata.artwork_url,
                    duration_ms: metadata.duration_ms
                }
            }
        });
    } catch (e) {
        console.error("Failed to play song:", e);
        isPlaying.set(false);
    }
}

export async function playStation(id: string, metadata: StationMetadata) {
    try {
        const firstTrack = await fetchStationFirstTrack(id);
        if (!firstTrack?.id) {
            throw new Error(`No playable tracks found for station ${id}`);
        }

        console.log(firstTrack, metadata);

        await playSong(firstTrack.id, {
            title: firstTrack.attributes?.name || metadata.name,
            artist: firstTrack.attributes?.artistName || metadata.subtitle || 'Station',
            album: firstTrack.attributes?.albumName || 'Apple Music Radio',
            artwork_url: formatArtworkUrl(firstTrack.attributes?.artwork, 600) || metadata.artwork_url,
            duration_ms: firstTrack.attributes?.durationInMillis,
        });
    } catch (e) {
        console.error('Failed to play station:', e);
        isPlaying.set(false);
    }
}

export async function togglePlayback() {
    // Optimistic toggle
    let currentPlaying = false;
    isPlaying.update(p => {
        currentPlaying = !p;
        return currentPlaying;
    });

    try {
        const playing = await invoke<boolean>('toggle_playback');
        isPlaying.set(playing);
    } catch (e) {
        console.error("Failed to toggle playback:", e);
        // Rollback on error
        isPlaying.set(!currentPlaying);
    }
}

export async function seekTo(seconds: number) {
    console.log("Seeking to:", seconds);
    try {
        // Update local state immediately for better responsiveness
        currentTime.set(seconds);
        await invoke('seek', { seconds });
    } catch (e) {
        console.error("Failed to seek:", e);
    }
}
