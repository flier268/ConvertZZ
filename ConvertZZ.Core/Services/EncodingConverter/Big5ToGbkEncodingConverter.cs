using System.Text;
using ConvertZZ.Core.Services.TextConverter;

namespace ConvertZZ.Core.Services.EncodingConverter
{
    public class Big5ToGbkEncodingConverter : CommonEncodingConverter
    {
        public Big5ToGbkEncodingConverter(ITextConverter textConverter)
        {
            TextConverter = textConverter;
        }

        public Big5ToGbkEncodingConverter(ITextConverter textConverter, IEncodingConverter converter) : base(converter)
        {
            TextConverter = textConverter;
        }

        public ITextConverter TextConverter { get; }
        private static Encoding GBK = Encoding.GetEncoding("GBK");
        private static Encoding BIG5 = Encoding.GetEncoding("BIG5");

        public override string Convert(string str, Encoding originEncoding, Encoding targetEncoding)
        {
            return Convert(str);
        }

        public string Convert(string str)
        {
            return base.Convert(TextConverter.Convert(str, Helpers.ETextConvertMode.T2S), BIG5, GBK);
        }

        public async Task<string> ConvertAsync(string str)
        {
            return base.Convert(await TextConverter.ConvertAsync(str, Helpers.ETextConvertMode.T2S), BIG5, GBK);
        }
    }
}