using System.Text;

namespace ConvertZZ.Core.Services.EncodingConverter
{
    public interface IEncodingConverter
    {
        public string Convert(string str, Encoding originEncoding, Encoding targetEncoding);
    }
}