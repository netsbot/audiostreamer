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

export interface QueueTrack {
    id: string;
    metadata: TrackMetadata;
}

interface PlaySongOptions {
    queue?: QueueTrack[];
    startIndex?: number;
    fromAutoNext?: boolean;
}

class PlaybackState {
    currentTrack = $state<TrackMetadata | null>(null);
    currentTrackId = $state<string | null>(null);
    isPlaying = $state(false);
    currentTime = $state(0);
    totalTime = $state(0);
    
    // -- High-precision interpolation --
    smoothTime = $state(0);
    private lastSyncRealTime = 0;
    private lastSyncPlaybackTime = 0;
    private rafId: number | null = null;

    private activeQueue: QueueTrack[] = [];
    private activeQueueIndex = -1;
    private autoAdvanceInFlight = false;

    constructor() {
        this.startInterpolationLoop();
    }

    private startInterpolationLoop() {
        const update = () => {
            if (this.isPlaying) {
                const now = performance.now();
                const delta = (now - this.lastSyncRealTime) / 1000;
                this.smoothTime = Math.max(0, Math.min(this.totalTime, this.lastSyncPlaybackTime + delta));
            } else {
                this.smoothTime = this.currentTime;
            }
            this.rafId = requestAnimationFrame(update);
        };
        this.rafId = requestAnimationFrame(update);
    }

    private syncTime(playbackTime: number) {
        this.currentTime = playbackTime;
        this.lastSyncPlaybackTime = playbackTime;
        this.lastSyncRealTime = performance.now();
        this.smoothTime = playbackTime;
    }

    private preloadNextTrack() {
        const nextIndex = this.activeQueueIndex + 1;
        if (nextIndex < 0 || nextIndex >= this.activeQueue.length) {
            return;
        }

        const next = this.activeQueue[nextIndex];
        invoke('preload_song', { adamId: next.id }).catch((error) => {
            console.debug('Failed to preload next track:', error);
        });
    }

    private async playNextInQueue() {
        if (this.autoAdvanceInFlight) return;
        const nextIndex = this.activeQueueIndex + 1;
        if (nextIndex < 0 || nextIndex >= this.activeQueue.length) {
            this.isPlaying = false;
            return;
        }

        this.autoAdvanceInFlight = true;
        try {
            const next = this.activeQueue[nextIndex];
            this.activeQueueIndex = nextIndex;
            await this.playSong(next.id, next.metadata, { fromAutoNext: true });
            this.preloadNextTrack();
        } finally {
            this.autoAdvanceInFlight = false;
        }
    }

    async initBridge() {
        console.log("Initializing playback bridge (Svelte 5 Smooth)...");

        await listen('playback-toggle', async () => {
            try {
                this.isPlaying = await invoke<boolean>('toggle_playback');
                if (this.isPlaying) {
                    this.lastSyncRealTime = performance.now();
                    this.lastSyncPlaybackTime = this.currentTime;
                }
            } catch (e) {
                console.error("Failed to toggle playback via bridge:", e);
            }
        });

        await listen('playback-progress', (event: any) => {
            const payload = event.payload as {
                currentTime?: number;
                current_time?: number;
                totalTime?: number;
                total_time?: number;
                paused?: boolean;
                ended?: boolean;
            };
            const current = payload.currentTime ?? payload.current_time ?? 0;
            const total = payload.totalTime ?? payload.total_time ?? 0;
            const paused = payload.paused ?? false;
            const ended = payload.ended ?? false;

            this.syncTime(current);
            if (total > 0) this.totalTime = total;
            this.isPlaying = !paused;

            if (ended) {
                void this.playNextInQueue();
            }
        });
    }

    async playSong(id: string, metadata: TrackMetadata, options: PlaySongOptions = {}) {
        try {
            if (options.queue && options.queue.length > 0) {
                this.activeQueue = options.queue;
                this.activeQueueIndex = options.startIndex ?? options.queue.findIndex((track) => track.id === id);
                if (this.activeQueueIndex < 0) this.activeQueueIndex = 0;
            } else if (!options.fromAutoNext) {
                this.activeQueue = [];
                this.activeQueueIndex = -1;
            }

            this.currentTrack = metadata;
            this.currentTrackId = id;
            this.isPlaying = true;
            this.totalTime = (metadata.duration_ms || 0) / 1000;
            this.syncTime(0);

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

            this.preloadNextTrack();
        } catch (e) {
            console.error("Failed to play song:", e);
            this.isPlaying = false;
        }
    }

    async togglePlayback() {
        const prev = this.isPlaying;
        this.isPlaying = !prev;
        if (this.isPlaying) {
            this.lastSyncRealTime = performance.now();
            this.lastSyncPlaybackTime = this.currentTime;
        }

        try {
            this.isPlaying = await invoke<boolean>('toggle_playback');
        } catch (e) {
            console.error("Failed to toggle playback:", e);
            this.isPlaying = prev;
        }
    }

    async seekTo(seconds: number) {
        this.syncTime(seconds);
        try {
            await invoke('seek', { seconds });
        } catch (e) {
            console.error("Failed to seek:", e);
        }
    }

    async playStation(id: string, metadata: StationMetadata) {
        try {
            const firstTrack = await this.fetchStationFirstTrack(id);
            if (!firstTrack?.id) throw new Error(`No tracks for station ${id}`);

            await this.playSong(firstTrack.id, {
                title: firstTrack.attributes?.name || metadata.name,
                artist: firstTrack.attributes?.artistName || metadata.subtitle || 'Station',
                album: firstTrack.attributes?.albumName || 'Apple Music Radio',
                artwork_url: this.formatArtworkUrl(firstTrack.attributes?.artwork, 600) || metadata.artwork_url,
                duration_ms: firstTrack.attributes?.durationInMillis,
            });
        } catch (e) {
            console.error('Failed to play station:', e);
            this.isPlaying = false;
        }
    }

    private formatArtworkUrl(artwork: any, size = 600): string | undefined {
        if (!artwork?.url) return undefined;
        return artwork.url.replace('{w}', `${size}`).replace('{h}', `${size}`).replace('{f}', 'webp').replace('{c}', '');
    }

    private async fetchStationFirstTrack(stationId: string): Promise<any> {
        const devToken = await invoke<string>('get_apple_music_token');
        const userToken = await invoke<string>('get_apple_music_user_token');
        const headers = { Authorization: `Bearer ${devToken}`, 'media-user-token': userToken, Origin: 'https://music.apple.com', Referer: 'https://music.apple.com/', 'Content-Type': 'application/json' };
        try {
            const response = await tauriFetch(`https://api.music.apple.com/v1/me/stations/next-tracks/${stationId}?limit=1`, { method: 'POST', headers });
            if (response.ok) {
                const payload: any = await response.json();
                return payload?.data?.[0];
            }
        } catch (e) { console.warn('Failed to fetch station next-track:', e); }
        return null;
    }
}

export const playback = new PlaybackState();
