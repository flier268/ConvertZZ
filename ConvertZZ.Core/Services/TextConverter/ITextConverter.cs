using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using ConvertZZ.Core.Helpers;

namespace ConvertZZ.Core.Services.TextConverter
{
    public interface ITextConverter
    {
        public string Convert(string text, ETextConvertMode mode);

        public Task<string> ConvertAsync(string text, ETextConvertMode mode);

        Task<string> GetVersion();

        Task<bool> IsEnable();
    }
}