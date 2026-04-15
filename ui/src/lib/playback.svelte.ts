import { listen } from '@tauri-apps/api/event';
import { fetchAppleMusic } from '$lib/appleMusic';
import { invoke } from '@tauri-apps/api/core';

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
    isStation?: boolean;
}

class PlaybackState {
    currentTrack = $state<TrackMetadata | null>(null);
    currentTrackId = $state<string | null>(null);
    isPlaying = $state(false);
    currentTime = $state(0);
    totalTime = $state(0);
    lyricsPaneOpen = $state(true);
    isShuffled = $state(false);
    repeatMode = $state(0); // 0: Off, 1: All, 2: One
    rightPaneMode = $state<'lyrics' | 'queue'>('lyrics');
    activeStationId = $state<string | null>(null);

    activeQueue = $state<QueueTrack[]>([]);
    originalQueue = $state<QueueTrack[]>([]);
    activeQueueIndex = $state(-1);

    // -- High-precision interpolation --
    smoothTime = $state(0);
    private lastSyncRealTime = 0;
    private lastSyncPlaybackTime = 0;
    private rafId: number | null = null;

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
        await this.playNext();
    }

    async playNext() {
        if (this.repeatMode === 2) {
            await this.seekTo(0);
            return;
        }

        const nextIndex = this.activeQueueIndex + 1;
        if (nextIndex < 0 || nextIndex >= this.activeQueue.length) {
            if (this.repeatMode === 1 && this.activeQueue.length > 0) {
                // Repeat All: Back to first song
                this.activeQueueIndex = 0;
                const next = this.activeQueue[0];
                await this.playSong(next.id, next.metadata, { fromAutoNext: true });
            } else {
                this.isPlaying = false;
            }
            return;
        }

        // Station Refill Check
        if (this.activeStationId && (this.activeQueue.length - nextIndex) < 3) {
            void this.refillStationQueue();
        }

        this.autoAdvanceInFlight = true;
        try {
            const next = this.activeQueue[nextIndex];
            this.activeQueueIndex = nextIndex;
            await this.playSong(next.id, next.metadata, { fromAutoNext: true });
        } finally {
            this.autoAdvanceInFlight = false;
        }
    }

    async playPrevious() {
        const prevIndex = this.activeQueueIndex - 1;
        if (prevIndex < 0) {
            await this.seekTo(0);
            return;
        }

        const prev = this.activeQueue[prevIndex];
        this.activeQueueIndex = prevIndex;
        await this.playSong(prev.id, prev.metadata, { fromAutoNext: true });
    }

    async initBridge() {
        console.log("Initializing playback bridge (Svelte 5 Smooth)...");

        await Promise.all([
            listen('playback-toggle', async () => {
                try {
                    this.isPlaying = await invoke<boolean>('toggle_playback');
                    if (this.isPlaying) {
                        this.lastSyncRealTime = performance.now();
                        this.lastSyncPlaybackTime = this.currentTime;
                    }
                } catch (e) {
                    console.error("Failed to toggle playback via bridge:", e);
                }
            }),

            listen('playback-progress', (event: any) => {
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
            }),

            listen('mpris-event', async (event: any) => {
                const type = event.payload as string;
                console.log("MPRIS event:", type);
                if (type === 'toggle') await this.togglePlayback();
                else if (type === 'next') await this.playNext();
                else if (type === 'previous') await this.playPrevious();
            })
        ]);
    }

    async playSong(id: string, metadata: TrackMetadata, options: PlaySongOptions = {}) {
        try {
            // Reset station mode if we are not playing from a station
            if (!options.isStation && !options.fromAutoNext) {
                this.activeStationId = null;
            }

            if (options.queue && options.queue.length > 0) {
                this.originalQueue = options.queue;
                if (this.isShuffled) {
                    const queueCopy = [...options.queue];
                    const currentIndex = options.startIndex ?? queueCopy.findIndex((track) => track.id === id);
                    const currentTrack = currentIndex >= 0 ? queueCopy.splice(currentIndex, 1)[0] : null;
                    const shuffled = this.shuffleArray(queueCopy);
                    this.activeQueue = currentTrack ? [currentTrack, ...shuffled] : shuffled;
                    this.activeQueueIndex = 0;
                } else {
                    this.activeQueue = options.queue;
                    this.activeQueueIndex = options.startIndex ?? options.queue.findIndex((track) => track.id === id);
                }
                if (this.activeQueueIndex < 0) this.activeQueueIndex = 0;
            } else if (!options.fromAutoNext) {
                this.activeQueue = [];
                this.originalQueue = [];
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

    private async refillStationQueue() {
        if (!this.activeStationId) return;
        try {
            console.log(`Refilling station queue for ${this.activeStationId}...`);
            const tracks = await this.fetchStationTracks(this.activeStationId, 1);
            if (tracks.length > 0) {
                // Check if track already in queue to avoid duplicates
                if (!this.activeQueue.some(t => t.id === tracks[0].id)) {
                    this.activeQueue = [...this.activeQueue, ...tracks];
                }
            }
        } catch (e) {
            console.error('Failed to refill station queue:', e);
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

    toggleLyricsPane() {
        this.lyricsPaneOpen = !this.lyricsPaneOpen;
    }

    toggleShuffle() {
        this.isShuffled = !this.isShuffled;
        if (this.activeQueue.length <= 1) return;

        if (this.isShuffled) {
            // Enabling shuffle
            this.originalQueue = [...this.activeQueue];
            const currentTrack = this.activeQueue[this.activeQueueIndex];
            const others = this.activeQueue.filter((_, i) => i !== this.activeQueueIndex);
            const shuffled = this.shuffleArray(others);
            this.activeQueue = [currentTrack, ...shuffled];
            this.activeQueueIndex = 0;
        } else {
            // Disabling shuffle
            const currentTrackId = this.currentTrackId;
            this.activeQueue = [...this.originalQueue];
            this.activeQueueIndex = this.activeQueue.findIndex(t => t.id === currentTrackId);
            if (this.activeQueueIndex < 0) this.activeQueueIndex = 0;
        }
    }

    toggleRepeat() {
        this.repeatMode = (this.repeatMode + 1) % 3;
    }

    toggleRightPaneMode() {
        this.rightPaneMode = this.rightPaneMode === 'lyrics' ? 'queue' : 'lyrics';
    }

    async removeFromQueue(index: number) {
        if (index < 0 || index >= this.activeQueue.length) return;

        // If removing the currently playing track, play next
        if (index === this.activeQueueIndex) {
            await this.playNext();
        }

        const removedItem = this.activeQueue.splice(index, 1)[0];

        // Adjust index if we removed something before current track
        if (index < this.activeQueueIndex) {
            this.activeQueueIndex--;
        }

        // Also remove from original queue if it exists there
        const origIdx = this.originalQueue.findIndex(t => t.id === removedItem.id);
        if (origIdx >= 0) {
            this.originalQueue.splice(origIdx, 1);
        }
    }

    async jumpToQueueIndex(index: number) {
        if (index < 0 || index >= this.activeQueue.length) return;
        const track = this.activeQueue[index];
        this.activeQueueIndex = index;
        await this.playSong(track.id, track.metadata, { fromAutoNext: true });
    }

    private shuffleArray<T>(array: T[]): T[] {
        const shuffled = [...array];
        for (let i = shuffled.length - 1; i > 0; i--) {
            const j = Math.floor(Math.random() * (i + 1));
            [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
        }
        return shuffled;
    }

    async playStation(id: string, metadata: StationMetadata) {
        try {
            this.activeStationId = id;
            const tracks = await this.fetchStationTracks(id, 1);
            if (tracks.length === 0) throw new Error(`No tracks for station ${id}`);

            const firstTrack = tracks[0];
            await this.playSong(firstTrack.id, firstTrack.metadata, {
                queue: tracks,
                startIndex: 0,
                isStation: true
            });

            // Pre-fetch one more for the queue
            void this.refillStationQueue();
        } catch (e) {
            console.error('Failed to play station:', e);
            this.isPlaying = false;
        }
    }

    private async fetchStationTracks(stationId: string, limit = 1): Promise<QueueTrack[]> {
        try {
            const response = await fetchAppleMusic(`https://api.music.apple.com/v1/me/stations/next-tracks/${stationId}?limit=${limit}`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                }
            });
            if (response.ok) {
                const payload: any = await response.json();
                const data = payload?.data || [];
                return data.map((item: any) => ({
                    id: item.id,
                    metadata: {
                        title: item.attributes?.name || 'Unknown',
                        artist: item.attributes?.artistName || 'Unknown',
                        album: item.attributes?.albumName || 'Unknown',
                        artwork_url: this.formatArtworkUrl(item.attributes?.artwork, 600),
                        duration_ms: item.attributes?.durationInMillis,
                    }
                }));
            }
        } catch (e) {
            console.warn('Failed to fetch station tracks:', e);
        }
        return [];
    }

    private formatArtworkUrl(artwork: any, size = 600): string | undefined {
        if (!artwork?.url) return undefined;
        return artwork.url.replace('{w}', `${size}`).replace('{h}', `${size}`).replace('{f}', 'webp').replace('{c}', '');
    }
}

export const playback = new PlaybackState();
