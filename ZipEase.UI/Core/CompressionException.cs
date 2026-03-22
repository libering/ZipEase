using System;

namespace ZipEase.UI.Core
{
    public class CompressionException : Exception
    {
        public int ErrorCode { get; }

        public CompressionException(string message, int errorCode) : base(message)
        {
            ErrorCode = errorCode;
        }
    }
}
