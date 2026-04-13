import { writable } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

export interface TrackMetadata {
    title: string;
    artist: string;
    album: string;
    artwork_url?: string;
    duration_ms?: number;
}

export const currentTrack = writable<TrackMetadata | null>(null);
export const isPlaying = writable(false);
export const currentTime = writable(0); // in seconds
export const totalTime = writable(0);   // in seconds

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
        const payload = event.payload as { currentTime: number; totalTime: number; paused: boolean };
        console.log("Playback progress received:", payload);
        console.debug("Playback progress:", payload.currentTime, "/", payload.totalTime);
        currentTime.set(payload.currentTime);
        if (payload.totalTime > 0) {
            totalTime.set(payload.totalTime);
        }
        isPlaying.set(!payload.paused);
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
            adamId: id, 
            metadata: {
                title: metadata.title,
                artist: metadata.artist,
                album: metadata.album,
                artwork_url: metadata.artwork_url,
                duration_ms: metadata.duration_ms
            } 
        });
    } catch (e) {
        console.error("Failed to play song:", e);
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
