using ConvertZZ.Core.Helpers;
using Fanhuaji_API;
using Fanhuaji_API.Enum;
using Fanhuaji_API.Models;

namespace ConvertZZ.Core.Services.TextConverter
{
    public class FanhuajiTextConverter : ITextConverter
    {
        public Config Config { get; private set; }
        private Fanhuaji Fanhuaji { get; set; }

        public FanhuajiTextConverter()
        {
            Config = new();
            Fanhuaji = new Fanhuaji(true, Fanhuaji.Terms_of_Service);
        }

        public string Convert(string text, ETextConvertMode mode)
        {
            return ConvertAsync(text, mode).Result;
        }

        public async Task<string> ConvertAsync(string text, ETextConvertMode mode)
        {
            Enum_Converter converter = Enum_Converter.Taiwan;
            switch (mode)
            {
                case ETextConvertMode.None:
                    return text;

                case ETextConvertMode.S2T:
                    converter = Enum_Converter.Traditional;
                    break;

                case ETextConvertMode.T2S:
                    converter = Enum_Converter.Simplified;
                    break;

                case ETextConvertMode.S2TW:
                    converter = Enum_Converter.Taiwan;
                    break;

                case ETextConvertMode.TW2S:
                    converter = Enum_Converter.China;
                    break;

                case ETextConvertMode.S2HK:
                    converter = Enum_Converter.Hongkong;
                    break;

                case ETextConvertMode.HK2S:
                    converter = Enum_Converter.Simplified;
                    break;

                case ETextConvertMode.S2TWP:
                    converter = Enum_Converter.Taiwan;
                    break;

                case ETextConvertMode.TW2SP:
                    converter = Enum_Converter.China;
                    break;

                case ETextConvertMode.T2TW:
                    converter = Enum_Converter.Taiwan;
                    break;

                case ETextConvertMode.T2HK:
                    converter = Enum_Converter.Hongkong;
                    break;

                default:
                    break;
            }
            var callback = await Fanhuaji.ConvertAsync(text, converter, Config);
            return callback.Data.Text;
        }

        public async Task<string> GetVersion()
        {
            var callback = await Fanhuaji.ConvertAsync("", Enum_Converter.Taiwan, Config);
            return callback.Revisions.Build;
        }

        public async Task<bool> IsEnable()
        {
            return await Task.FromResult(Fanhuaji.CheckConnection());
        }

        public FanhuajiTextConverter SetConfig(Config config)
        {
            Config = config;
            return this;
        }
    }
}