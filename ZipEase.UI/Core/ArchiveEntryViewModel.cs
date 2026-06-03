using System.Windows.Media.Imaging;
using CommunityToolkit.Mvvm.ComponentModel;

namespace ZipEase.UI.Core
{
    public partial class ArchiveEntryViewModel : ObservableObject
    {
        [ObservableProperty] private string _fileName = string.Empty;
        [ObservableProperty] private string _fileType = string.Empty;
        [ObservableProperty] private string _formattedSize = string.Empty;
        [ObservableProperty] private string _icon = "Document24";
        [ObservableProperty] private WriteableBitmap? _thumbnail;
        private bool _isDirectory;

        public bool IsDirectory => _isDirectory;
        public bool IsFile => !_isDirectory;
        public long SizeBytes { get; private set; }

        /// <summary>
        /// Whether this entry is a previewable image (based on extension).
        /// Used to show a preview icon in the file list and to trigger thumbnail generation.
        /// </summary>
        public bool IsPreviewable { get; private set; }

        /// <summary>
        /// Whether a thumbnail has been loaded (non-null) for this entry.
        /// When false, the file list shows the placeholder icon.
        /// </summary>
        public bool HasThumbnail => Thumbnail != null;

        public ArchiveEntryViewModel() { }

        public ArchiveEntryViewModel(ArchiveEntry entry)
        {
            _fileName = entry.FileName;
            _fileType = entry.FileType;
            _formattedSize = entry.FormattedSize;
            _isDirectory = entry.IsDirectory;
            SizeBytes = entry.Size;
            IsPreviewable = ThumbnailService.IsPreviewable(entry.FileName, entry.IsDirectory);
            _icon = entry.IsDirectory
                ? "Folder24"
                : IsPreviewable ? "Image24" : GetIconForType(entry.FileType);
        }

        /// <summary>
        /// Notifies the UI that the thumbnail availability has changed.
        /// Called after <see cref="Thumbnail"/> is set.
        /// </summary>
        partial void OnThumbnailChanged(WriteableBitmap? value)
        {
            OnPropertyChanged(nameof(HasThumbnail));
        }

        private static string GetIconForType(string fileType) => fileType.ToUpperInvariant() switch
        {
            "ZIP" or "RAR" or "7Z" or "TAR" or "GZ" => "Archive24",
            "PDF" => "DocumentPdf24",
            "TXT" or "MD" => "DocumentText24",
            "JPG" or "JPEG" or "PNG" or "GIF" or "BMP" => "Image24",
            "MP3" or "WAV" or "FLAC" => "MusicNote124",
            "MP4" or "AVI" or "MKV" => "Video24",
            _ => "Document24"
        };
    }
}
