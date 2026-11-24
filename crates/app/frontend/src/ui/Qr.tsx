/**
 * An image the backend drew: a `data:` URL (share.rs gives the QR as an SVG data URL and the
 * beta marks as PNG data URLs), or raw SVG markup, which becomes a data URL. Never inserted
 * as markup, so nothing in it runs.
 */
export function imageSource(image: string): string {
  return image.startsWith("data:") ? image : `data:image/svg+xml;charset=utf-8,${encodeURIComponent(image)}`;
}

/** A QR code, always on white so phones read it in both themes. */
export function Qr(props: { image: string; label: string }) {
  return (
    <div className="shrink-0 rounded-[10px] bg-white p-2 shadow-[0_0_0_0.5px_rgb(0_0_0/0.12)]">
      <img src={imageSource(props.image)} alt={props.label} className="size-[104px]" />
    </div>
  );
}
