import { playback } from './playback.svelte';

export type ViewMode = 'home' | 'search' | 'album' | 'playlist' | 'library' | 'browse' | 'radio';

class NavigationState {
  activeView = $state<ViewMode>('home');
  selectedPlaylistId = $state('');
  selectedPlaylistType = $state('playlists');
  selectedAlbumId = $state('');

  openPlaylist(id: string, type = 'playlists') {
    this.selectedPlaylistId = id;
    this.selectedPlaylistType = type;
    this.activeView = 'playlist';
  }

  openAlbum(id: string) {
    this.selectedAlbumId = id;
    this.activeView = 'album';
  }

  goHome() {
    this.activeView = 'home';
  }

  goBack() {
    // Current simple logic for back button in MainFeed
    this.activeView = 'home';
  }
}

export const navigation = new NavigationState();
