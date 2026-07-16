const LOADED_CLASS = "image-loaded";

type ImageSource = string | null | undefined;

function sourceValue(source: ImageSource): string {
  return source ?? "";
}

function ownsCurrentSource(node: HTMLImageElement, source: string): boolean {
  return (node.getAttribute("src") ?? "") === source;
}

/**
 * Keep media art transparent until the browser has successfully decoded enough
 * image data to report a real intrinsic width. The source value is explicit so
 * a reused <img> cannot inherit the loaded state of its previous URL.
 */
export function imageReveal(node: HTMLImageElement, resolvedSource: ImageSource) {
  let source = sourceValue(resolvedSource);
  let generation = 0;
  let destroyed = false;

  const hide = () => node.classList.remove(LOADED_CLASS);

  const revealIfReady = (ownedGeneration: number, ownedSource: string) => {
    if (
      destroyed ||
      generation !== ownedGeneration ||
      !ownsCurrentSource(node, ownedSource)
    ) {
      return;
    }

    if (node.complete && node.naturalWidth > 0) {
      node.classList.add(LOADED_CLASS);
    } else {
      hide();
    }
  };

  const queueCachedCheck = () => {
    const ownedGeneration = generation;
    const ownedSource = source;
    queueMicrotask(() => revealIfReady(ownedGeneration, ownedSource));
  };

  const onLoad = () => revealIfReady(generation, source);
  const onError = () => {
    if (!destroyed && ownsCurrentSource(node, source)) hide();
  };

  const reset = (nextSource: ImageSource) => {
    generation += 1;
    source = sourceValue(nextSource);
    hide();
    queueCachedCheck();
  };

  node.addEventListener("load", onLoad);
  node.addEventListener("error", onError);
  reset(resolvedSource);

  return {
    update(nextSource: ImageSource) {
      if (sourceValue(nextSource) !== source) reset(nextSource);
    },
    destroy() {
      destroyed = true;
      generation += 1;
      node.removeEventListener("load", onLoad);
      node.removeEventListener("error", onError);
    },
  };
}
