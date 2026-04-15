interface SnapShelfOptions {
  minItemWidth?: number;
  maxItemWidth?: number;
  targetItemWidth?: number;
}

const DEFAULT_MIN = 176;
const DEFAULT_MAX = 240;
const DEFAULT_TARGET = 224;

function parsePx(value: string): number {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

export function snapShelf(node: HTMLElement, options: SnapShelfOptions = {}) {
  let resizeObserver: ResizeObserver | null = null;

  const update = () => {
    const styles = getComputedStyle(node);
    const paddingLeft = parsePx(styles.paddingLeft);
    const paddingRight = parsePx(styles.paddingRight);
    const gap = parsePx(styles.columnGap || styles.gap);

    const minItemWidth = options.minItemWidth ?? DEFAULT_MIN;
    const maxItemWidth = options.maxItemWidth ?? DEFAULT_MAX;
    const targetItemWidth = options.targetItemWidth ?? DEFAULT_TARGET;

    const usableWidth = Math.max(0, node.clientWidth - paddingLeft - paddingRight);
    const idealColumns = Math.max(1, Math.round((usableWidth + gap) / (targetItemWidth + gap)));

    const snapped = Math.floor((usableWidth - gap * (idealColumns - 1)) / idealColumns);
    const snappedWidth = Math.max(minItemWidth, Math.min(maxItemWidth, snapped));

    node.style.setProperty("--snap-item-width", `${Math.round(snappedWidth)}px`);
    node.style.scrollPaddingLeft = `${Math.round(paddingLeft)}px`;
    node.style.scrollPaddingRight = `${Math.round(paddingRight)}px`;
  };

  resizeObserver = new ResizeObserver(() => {
    update();
  });

  resizeObserver.observe(node);
  update();

  return {
    update(nextOptions: SnapShelfOptions = {}) {
      options = nextOptions;
      update();
    },
    destroy() {
      resizeObserver?.disconnect();
      resizeObserver = null;
    },
  };
}
