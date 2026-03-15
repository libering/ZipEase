using System;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// Exception thrown when archive extraction fails.
    /// </summary>
    public class ExtractionException : Exception
    {
        /// <summary>
        /// Gets the error code returned from the Rust core.
        /// </summary>
        public int ErrorCode { get; }

        /// <summary>
        /// Initializes a new instance of the ExtractionException class.
        /// </summary>
        /// <param name="message">The error message.</param>
        /// <param name="errorCode">The error code from Rust core.</param>
        public ExtractionException(string message, int errorCode) : base(message)
        {
            ErrorCode = errorCode;
        }
    }
}
