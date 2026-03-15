using System;
using System.Runtime.InteropServices;

namespace ZipEase.UI.Core
{
    public class LockException : Exception
    {
        public int ErrorCode { get; }
        public LockException(string message, int errorCode) : base(message)
        {
            ErrorCode = errorCode;
        }
    }

    public class DirectoryLock : IDisposable
    {
        private IntPtr _handle;
        private bool _disposed;

        internal DirectoryLock(IntPtr handle)
        {
            _handle = handle;
        }

        public void Dispose()
        {
            Dispose(true);
            GC.SuppressFinalize(this);
        }

        protected virtual void Dispose(bool disposing)
        {
            if (!_disposed)
            {
                if (_handle != IntPtr.Zero && _handle != new IntPtr(-1))
                {
                    NativeMethods.UnlockDirectory(_handle);
                    _handle = IntPtr.Zero;
                }
                _disposed = true;
            }
        }

        ~DirectoryLock()
        {
            Dispose(false);
        }
    }

    public static class DirectoryLockManager
    {
        public static DirectoryLock Lock(string path)
        {
            if (string.IsNullOrEmpty(path))
                throw new ArgumentException("Path cannot be null or empty", nameof(path));

            IntPtr handle = NativeMethods.LockDirectory(path);
            if (handle == new IntPtr(-1))
            {
                string errorMessage = GetLastErrorMessage();
                throw new LockException(errorMessage ?? "Unknown error occurred during locking", -1);
            }

            return new DirectoryLock(handle);
        }

        public static void Unlock(DirectoryLock directoryLock)
        {
            directoryLock?.Dispose();
        }

        private static string GetLastErrorMessage()
        {
            IntPtr ptr = NativeMethods.GetLastError();
            if (ptr == IntPtr.Zero) return "Unknown error";

            string? message = Marshal.PtrToStringUni(ptr);
            NativeMethods.FreeErrorString(ptr);
            return message ?? "Unknown error";
        }
    }
}
