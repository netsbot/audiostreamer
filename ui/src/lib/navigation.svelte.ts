import { playback } from './playback.svelte';

export type ViewMode = 'home' | 'search' | 'album' | 'playlist';

class NavigationState {
  activeView = $state<ViewMode>('home');
  selectedPlaylistId = $state('');
  selectedPlaylistType = $state('playlists');
  selectedPlaylistHref = $state('');
  selectedAlbumId = $state('');
  selectedAlbumType = $state('albums');
  selectedAlbumHref = $state('');

  openPlaylist(id: string, type = 'playlists', href = '') {
    this.selectedPlaylistId = id;
    this.selectedPlaylistType = type;
    this.selectedPlaylistHref = href;
    this.activeView = 'playlist';
  }

  openAlbum(id: string, type = 'albums', href = '') {
    this.selectedAlbumId = id;
    this.selectedAlbumType = type;
    this.selectedAlbumHref = href;
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
