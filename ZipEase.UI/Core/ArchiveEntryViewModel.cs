using CommunityToolkit.Mvvm.ComponentModel;

namespace ZipEase.UI.Core
{
    public partial class ArchiveEntryViewModel : ObservableObject
    {
        [ObservableProperty] private string _fileName = string.Empty;
        [ObservableProperty] private string _fileType = string.Empty;
        [ObservableProperty] private string _formattedSize = string.Empty;
        [ObservableProperty] private string _icon = "Document24";
        private bool _isDirectory;

        public bool IsDirectory => _isDirectory;
        public bool IsFile => !_isDirectory;

        public ArchiveEntryViewModel() { }

        public ArchiveEntryViewModel(ArchiveEntry entry)
        {
            _fileName = entry.FileName;
            _fileType = entry.FileType;
            _formattedSize = entry.FormattedSize;
            _isDirectory = entry.IsDirectory;
            _icon = entry.IsDirectory ? "Folder24" : GetIconForType(entry.FileType);
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
