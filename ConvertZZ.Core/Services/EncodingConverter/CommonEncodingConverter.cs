using System.Text;

namespace ConvertZZ.Core.Services.EncodingConverter
{
    public class CommonEncodingConverter
    {
        private IEncodingConverter? _converter;

        public CommonEncodingConverter()
        {
            _converter = new LocalEncodingConverter();
        }

        public CommonEncodingConverter(IEncodingConverter converter)
        {
            _converter = converter;
        }

        public CommonEncodingConverter SetConverterEngine(IEncodingConverter encodingConverter)
        {
            _converter = encodingConverter;
            return this;
        }

        public virtual string Convert(string str, Encoding originEncoding, Encoding targetEncoding)
        {
            if (_converter is null)
                throw new NullReferenceException($"{nameof(_converter)} should not null!");
            return _converter.Convert(str, originEncoding, targetEncoding);
        }
    }
}