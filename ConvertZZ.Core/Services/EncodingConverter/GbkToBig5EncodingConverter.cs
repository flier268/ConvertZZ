using System.Text;
using ConvertZZ.Core.Services.TextConverter;

namespace ConvertZZ.Core.Services.EncodingConverter
{
    public class GbkToBig5EncodingConverter : CommonEncodingConverter
    {
        public GbkToBig5EncodingConverter(ITextConverter textConverter)
        {
            TextConverter = textConverter;
        }

        public GbkToBig5EncodingConverter(ITextConverter textConverter, IEncodingConverter converter) : base(converter)
        {
            TextConverter = textConverter;
        }

        public ITextConverter TextConverter { get; }
        private static Encoding GBK = Encoding.GetEncoding("GBK");
        private static Encoding BIG5 = Encoding.GetEncoding("BIG5");

        [Obsolete]
        public new string Convert(string str, Encoding originEncoding, Encoding targetEncoding)
        {
            return TextConverter.Convert(base.Convert(str, originEncoding, targetEncoding), Helpers.ETextConvertMode.S2T);
        }

        public string Convert(string str)
        {
            return TextConverter.Convert(base.Convert(str, GBK, BIG5), Helpers.ETextConvertMode.S2T);
        }

        public async Task<string> ConvertAsync(string str)
        {
            return await TextConverter.ConvertAsync(base.Convert(str, GBK, BIG5), Helpers.ETextConvertMode.S2T);
        }
    }
}