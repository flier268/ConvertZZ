using System;
using System.IO;
using System.Text;
using System.Threading.Tasks;

namespace ConvertZZ.Moudle
{
    public class StreamString
    {
        private Stream ioStream;
        private UnicodeEncoding streamEncoding;

        public StreamString(Stream ioStream)
        {
            this.ioStream = ioStream;
            streamEncoding = new UnicodeEncoding();
        }

        public string ReadString()
        {
            int len;
            len = ioStream.ReadByte() * 256;
            len += ioStream.ReadByte();
            byte[] inBuffer = new byte[len];
            ioStream.Read(inBuffer, 0, len);
            return streamEncoding.GetString(inBuffer);
        }
        public async Task<string> ReadStringAsync()
        {
            int len;
            len = ioStream.ReadByte() * 256;
            len += ioStream.ReadByte();
            byte[] inBuffer = new byte[len];
            await ioStream.ReadAsync(inBuffer, 0, len);
            return streamEncoding.GetString(inBuffer);
        }
        public int WriteString(string outString)
        {
            byte[] outBuffer = streamEncoding.GetBytes(outString);
            int len = outBuffer.Length;
            if (len > UInt16.MaxValue)
            {
                len = (int)UInt16.MaxValue;
            }
            ioStream.WriteByte((byte)(len / 256));
            ioStream.WriteByte((byte)(len & 255));
            ioStream.Write(outBuffer, 0, len);
            ioStream.Flush();
            return outBuffer.Length + 2;
        }
        public async Task<int> WriteStringAsync(string outString)
        {
            byte[] outBuffer = streamEncoding.GetBytes(outString);
            int len = outBuffer.Length;
            if (len > UInt16.MaxValue)
            {
                len = (int)UInt16.MaxValue;
            }
            await ioStream.WriteAsync(new byte[] { (byte)(len / 256) }, 0, 1);
            await ioStream.WriteAsync(new byte[] { (byte)(len & 255) }, 0, 1);
            await ioStream.WriteAsync(outBuffer, 0, len);
            await ioStream.FlushAsync();
            return outBuffer.Length + 2;
        }
    }
}
