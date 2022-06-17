using System.Text;

namespace ConvertZZ.Core.Services.EncodingConverter
{
    public class LocalEncodingConverter : IEncodingConverter
    {
        public string Convert(string str, Encoding originEncoding, Encoding targetEncoding)
        {
            if (string.IsNullOrWhiteSpace(str)) return str;
            return originEncoding.GetString(targetEncoding.GetBytes(str));
        }
    }
}