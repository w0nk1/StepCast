type ImageLightboxProps = {
  src: string;
  alt: string;
  onClose: () => void;
};

export default function ImageLightbox({ src, alt, onClose }: ImageLightboxProps) {
  return (
    <div className="editor-lightbox-overlay" onClick={onClose}>
      <button className="editor-lightbox-close" onClick={onClose} title="Close lightbox">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <path d="M18 6L6 18M6 6l12 12" />
        </svg>
      </button>
      <img
        className="editor-lightbox-img"
        src={src}
        alt={alt}
        onClick={(e) => e.stopPropagation()}
        draggable={false}
      />
    </div>
  );
}
